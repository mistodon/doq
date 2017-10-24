extern crate ansi_term;
extern crate chrono;
extern crate clap;
extern crate close_enough;
extern crate serde;

#[macro_use]
extern crate serde_derive;

extern crate serde_yaml;


use std::path::{ Path, PathBuf };
use ansi_term::Color;
use chrono::{ Utc, NaiveDate };
use serde::{ Serialize, Deserialize };


fn fail(message: &str) -> !
{
    eprintln!("doq: error: {}", message);
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
    pub frequency_days: u64,
    last_completed: Option<String>
}

impl Task
{
    pub fn new(name: &str, frequency_days: u64, last_completed: Option<NaiveDate>) -> Self
    {
        let mut task = Task { name: name.to_owned(), frequency_days, last_completed: None };
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

    let app = App::new("doq")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Tool for tracking tasks which need done regularly.")
        .arg(
            Arg::with_name("file")
                .help("The schedule file to read and write from. Defaults to the file specified in ~/.doq")
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
                    Arg::with_name("frequency")
                        .help("How frequently this task should be done in days")
                        .short("f")
                        .long("frequency")
                        .takes_value(true)
                        .required(true)
                    )
                .arg(
                    Arg::with_name("done")
                        .help("Set if the task was done today")
                        .long("done")
                    )
            )
        .subcommand(
            SubCommand::with_name("remove")
                .about("Stop tracking a task")
                .arg(
                    Arg::with_name("name")
                        .help("The name of the task to remove")
                        .takes_value(true)
                        .required(true)
                    )
            )
        .subcommand(
            SubCommand::with_name("did")
                .about("Mark a task as done")
                .arg(
                    Arg::with_name("task")
                        .help("The name of the task to mark done. Fuzzily matched.")
                        .takes_value(true)
                        .required(true)
                    )
                .arg(
                    Arg::with_name("on")
                        .help("Specify the date of completion")
                        .long("on")
                        .takes_value(true)
                    )
                .arg(
                    Arg::with_name("yes")
                        .help("Bypass confirmation prompt")
                        .short("y")
                    )
            );

    let matches = app.get_matches();

    let schedule_file = &{
        match matches.value_of("file")
        {
            Some(file) => PathBuf::from(file),
            None => {
                let home = std::env::home_dir().or_fail("Failed to find home directory");
                let dotfile_path: PathBuf = [home.as_path(), ".doq".as_ref()].iter().collect();
                let default_schedule_path: PathBuf = [home.as_path(), ".doq_schedule".as_ref()].iter().collect();

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
            let freq: u64 = matches.value_of("frequency").unwrap().parse().or_fail("Frequency must be a number of days");
            let done = matches.is_present("done");

            if schedule.tasks.iter().find(|t| t.name == name).is_some()
            {
                fail("Task already exists");
            }

            let date = if done { Some(Utc::today().naive_utc()) } else { None };

            schedule.tasks.push(Task::new(name, freq, date));
            write_file(schedule_file, &schedule);
        },

        ("remove", Some(matches)) =>
        {
            let name = matches.value_of("name").unwrap();
            let index = schedule.tasks.iter().position(|t| t.name == name).or_fail("No task with that name");
            schedule.tasks.swap_remove(index);
            write_file(schedule_file, &schedule);
        }

        ("did", Some(matches)) =>
        {
            use std::str::FromStr;

            let name = matches.value_of("task").unwrap();
            let date = match matches.value_of("on")
            {
                Some(date) => NaiveDate::from_str(date).or_fail("Invalid date format"),
                None => Utc::today().naive_utc()
            };
            let yes = matches.is_present("yes");

            let should_write = {
                let task_name = close_enough::close_enough(schedule.tasks.iter().map(|t| &t.name), name).or_fail("No task matching that name").to_owned();
                let task = schedule.tasks.iter_mut().find(|t| t.name == task_name).unwrap();

                let proceed = match yes
                {
                    true => true,
                    false =>
                    {
                        println!("Mark task '{}' as done on {}? (y/N) ", task.name, date);
                        let mut buffer = String::new();
                        std::io::stdin().read_line(&mut buffer).or_fail("Failed to read from stdin");

                        let command = buffer.trim().to_lowercase();
                        (command == "y" || command == "yes")
                    }
                };
          
                if proceed
                {
                    task.set_last_completed(date);
                    true
                }
                else
                {
                    eprintln!("Cancelling");
                    false
                }
            };

            if should_write
            {
                write_file(schedule_file, &schedule);
            }
        },
        
        _ => ()
    }

    {
        // TODO: Stretch column sizes to fit max item
        println!("{: <20}      {: <16}", "Task", "Last completed");
        println!("{: <20}      {: <16}", "===", "===");

        let mut delta_tasks: Vec<_> = schedule.tasks.iter().map(
            |task| match task.last_completed()
            {
                Some(date) =>
                {
                    let days = Utc::today().naive_utc().signed_duration_since(date).num_days();
                    let delta = days - (task.frequency_days as i64);
                    (delta, task)
                },
                None => (std::i64::MAX, task)
            }).collect();
        delta_tasks.sort_by_key(|&(delta, _)| -delta);

        let red = Color::Fixed(9);
        let green = Color::Fixed(10);
        let yellow = Color::Fixed(11);

        for &(delta, task) in &delta_tasks
        {
            let freq_string = format!("{}d", task.frequency_days);

            let (line, color) = match task.last_completed()
            {
                Some(date) =>
                {
                    let days = Utc::today().naive_utc().signed_duration_since(date).num_days();
                    let datestring = date.to_string();

                    let days_ago_text = match days
                    {
                        0 => "    Today".to_owned(),
                        1 => "  1 day ago".to_owned(),
                        n => format!("{: >3} days ago", n)
                    };

                    let (color, status) = match delta
                    {
                        delta if delta < 0 =>
                            (green, format!("(Due in {} days)", -delta)),
                            delta if delta == 0 =>
                                (yellow, format!("(Due today)")),
                            delta =>
                                (red, format!("({} days overdue!)", delta))
                    };

                    let line = format!("{: <20} {: >3}  {: <16} {: <16} {}", task.name, freq_string, datestring, days_ago_text, status);
                    (line, color)
                },
                None => (format!("{: <20} {: >3}  {: <16}", task.name, freq_string, "Never"), red)
            };

            println!("{}", color.paint(line));
        }
    }
}
