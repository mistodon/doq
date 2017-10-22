extern crate chrono;
extern crate clap;
extern crate serde;

#[macro_use]
extern crate serde_derive;

extern crate serde_yaml;


use std::path::{ Path, PathBuf };
use chrono::{ Utc, NaiveDate };
use serde::{ Serialize, Deserialize };


fn fail(message: &str) -> !
{
    eprintln!("clockq: error: {}", message);
    std::process::exit(1);
}


trait OrFail<T>
{
    fn or_fail(self, message: &str) -> T;
}

impl<T, E> OrFail<T> for Result<T, E>
{
    fn or_fail(self, message: &str) -> T
    {
        self.unwrap_or_else(|_| fail(message))
    }
}

impl<T> OrFail<T> for Option<T>
{
    fn or_fail(self, message: &str) -> T
    {
        self.unwrap_or_else(|| fail(message))
    }
}


#[derive(Debug, Serialize, Deserialize)]
struct AppConfig
{
    pub schedule_file: PathBuf
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Schedule
{
    pub tasks: Vec<Task>
}

#[derive(Debug, Serialize, Deserialize)]
struct Task
{
    pub name: String,
    last_completed: Option<String>
}

impl Task
{
    pub fn new(name: &str, last_completed: Option<NaiveDate>) -> Self
    {
        let mut task = Task { name: name.to_owned(), last_completed: None };
        if let Some(date) = last_completed
        {
            task.set_last_completed(date);
        }
        task
    }

    pub fn set_last_completed(&mut self, date: NaiveDate)
    {
        let datestring = format!("{}", date);
        self.last_completed = Some(datestring);
    }

    pub fn last_completed(&self) -> Option<NaiveDate>
    {
        use std::str::FromStr;

        self.last_completed.as_ref().map(|s| NaiveDate::from_str(s).or_fail("Failed to parse date"))
    }
}


fn ensure_file_exists<T: Serialize>(path: &Path, default_content: &T)
{
    use std::fs::File;

    if !path.exists()
    {
        let file = &mut File::create(path).or_fail("Failed to create file");
        serde_yaml::to_writer(file, default_content).or_fail("Failed to write to file");
    }
}

fn read_file<T>(path: &Path) -> T
where for <'de>
    T: Deserialize<'de>
{
    use std::fs::File;

    let file = &File::open(path).or_fail("Failed to read file");
    serde_yaml::from_reader(file).or_fail("Failed to parse file")
}

fn write_file<T: Serialize>(path: &Path, data: &T)
{
    use std::fs::File;

    let file = &mut File::create(path).or_fail("Failed to open file");
    serde_yaml::to_writer(file, data).or_fail("Failed to write to file");
}


fn main()
{
    use clap::{App, SubCommand, Arg};

    let app = App::new("clap")
        .version(env!("CARGO_PKG_VERSION"))
        .about("MISSING ABOUT DESCRIPTION!!!")
        .arg(
            Arg::with_name("file")
                .help("The schedule file to read and write from. Defaults to the file specified in ~/.clockq")
                .long("file")
                .short("f")
                .takes_value(true)
            )
        .subcommand(
            SubCommand::with_name("add")
                .about("Add a task to track")
                .arg(
                    Arg::with_name("name")
                        .help("The name of the task to track")
                        .takes_value(true)
                        .required(true)
                    )
                .arg(
                    Arg::with_name("done")
                        .help("Set if the task was done today")
                        .long("done")
                    )
            );

    let matches = app.get_matches();

    let schedule_file = &{
        match matches.value_of("file")
        {
            Some(file) => PathBuf::from(file),
            None => {
                let home = std::env::home_dir().or_fail("Failed to find home directory");
                let dotfile_path: PathBuf = [home.as_path(), ".clockq".as_ref()].iter().collect();
                let default_schedule_path: PathBuf = [home.as_path(), ".clockq_schedule".as_ref()].iter().collect();

                ensure_file_exists(&dotfile_path, &AppConfig { schedule_file: default_schedule_path });
                let config: AppConfig = read_file(&dotfile_path);

                PathBuf::from(config.schedule_file)
            }
        }
    };

    ensure_file_exists(schedule_file, &Schedule::default());

    let mut schedule: Schedule = read_file(schedule_file);

    match matches.subcommand()
    {
        ("add", Some(matches)) =>
        {
            let name = matches.value_of("name").unwrap();
            let done = matches.is_present("done");

            if schedule.tasks.iter().find(|t| t.name == name).is_some()
            {
                fail("Task already exists");
            }

            let date = if done { Some(Utc::today().naive_utc()) } else { None };

            schedule.tasks.push(Task::new(name, date));
            write_file(schedule_file, &schedule);
        },
        _ =>
        {
            println!("{: <20} {: <16}", "Task", "Last completed");
            println!("{: <20} {: <16}", "===", "===");
            for task in schedule.tasks.iter()
            {
                let (datestring, days_ago_text) = match task.last_completed()
                {
                    Some(date) =>
                    {
                        let days = date.signed_duration_since(Utc::today().naive_utc()).num_days();
                        (date.to_string(), format!("{} days ago", days))
                    }
                    None => ("Never".to_owned(), "".to_owned())
                };
                println!("{: <20} {: <16} {}", task.name, datestring, days_ago_text);
            }
        }
    }
}
