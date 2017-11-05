extern crate ansi_term;
extern crate clap;
extern crate close_enough;
extern crate serde;

#[macro_use]
extern crate serde_derive;

extern crate serde_yaml;


extern crate doq;


use std::path::{ Path, PathBuf };
use ansi_term::Color;
use serde::{ Serialize, Deserialize };
use doq::data::*;


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


fn main()
{
    use clap::{App, SubCommand, Arg, AppSettings};

    // TODO: Add validator for --repeat option
    // TODO: Add validator for dates
    // TODO: Fuzzy matching on all commands
    // TODO: Confirmation prompt on all destructive actions
    // TODO: Add edit subcommand (like add, but fuzzy-matched and keeps unspecified options)
    // TODO: Add flags to limit what is shown in schedule
    let app = App::new("doq")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Tool for tracking tasks which need done regularly.")
        .settings(&[AppSettings::VersionlessSubcommands])
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
                    Arg::with_name("on")
                        .help("The date this task is due to be completed on")
                        .takes_value(true)
                        .long("on")
                    )
                .arg(
                    Arg::with_name("repeat")
                        .help("How frequently this task should repeat")
                        .short("r")
                        .long("repeat")
                        .takes_value(true)
                        .required(true)
                    )
                .arg(
                    Arg::with_name("at_least")
                        .help("Specify that the repeat period is relative to completion date rather than due date")
                        .long("at-least")
                    )
            )

        .subcommand(
            SubCommand::with_name("edit")
                .about("Modify an existing task")
                .arg(
                    Arg::with_name("name")
                        .help("The name of the task to modify. Fuzzily matched.")
                        .takes_value(true)
                        .required(true)
                    )
                .arg(
                    Arg::with_name("rename")
                        .help("New name for the task")
                        .takes_value(true)
                        .long("rename")
                    )
                .arg(
                    Arg::with_name("on")
                        .help("New due-date of the task")
                        .takes_value(true)
                        .long("on")
                    )
                .arg(
                    Arg::with_name("repeat")
                        .help("How frequently this task should now repeat")
                        .short("r")
                        .long("repeat")
                        .takes_value(true)
                    )
                .arg(
                    Arg::with_name("at_least")
                        .help("Whether or not this task repeats relative to completion date rather than due date")
                        .long("at-least")
                        .takes_value(true)
                        .possible_values(&["true", "false"])
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
                #[cfg(debug_assertions)]
                {
                    PathBuf::from(".doq_schedule")
                }

                #[cfg(not(debug_assertions))]
                {
                    let home = std::env::home_dir().or_fail("Failed to find home directory");
                    let dotfile_path: PathBuf = [home.as_path(), ".doq".as_ref()].iter().collect();
                    let default_schedule_path: PathBuf = [home.as_path(), ".doq_schedule".as_ref()].iter().collect();

                    ensure_file_exists(&dotfile_path, &AppConfig { schedule_file: default_schedule_path });
                    let config: AppConfig = read_file(&dotfile_path);

                    PathBuf::from(config.schedule_file)
                }
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
            let repeat = {
                let value = matches.value_of("repeat").unwrap();

                if value == "never"
                {
                    Repeat::Never
                }
                else
                {
                    let (count, unit) = value.split_at(value.len() - 1);
                    let count: u32 = count.parse().or_fail("Expected a number");

                    match unit
                    {
                        "d" => Repeat::Days(count),
                        "m" => Repeat::Months(count),
                        "y" => Repeat::Years(count),
                        _ => fail("Expected a suffix (d, m, y) for days, months, or years")
                    }
                }
            };

            let at_least = matches.is_present("at_least");

            if repeat == Repeat::Never && at_least
            {
                fail("Cannot specify --at-least and --repeat never");
            }

            if schedule.tasks.iter().find(|t| t.name == name).is_some()
            {
                fail("Task already exists");
            }

            let date_due = parse_date_or_today(matches.value_of("on"));

            schedule.tasks.push(
                Task
                {
                    name: name.to_owned(),
                    repeat,
                    date_completed: None,
                    date_due: date_due.into(),
                    at_least
                });

            write_file(schedule_file, &schedule);
        },

        ("edit", Some(matches)) =>
        {
            let name = matches.value_of("name").unwrap();

            let task_name = close_enough::close_enough(schedule.tasks.iter().map(|t| &t.name), name).or_fail("No task matching that name").to_owned();

            let task = schedule.tasks.iter_mut().find(|t| t.name == task_name).unwrap();

            if let Some(new_name) = matches.value_of("rename")
            {
                task.name = new_name.to_owned();
            }

            if let Some(on) = matches.value_of("on")
            {
                task.date_due = parse_date(on).into();
            }

            if let Some(repeat) = matches.value_of("repeat")
            {
                task.repeat = doq::repeat_from_string(repeat).unwrap_or_else(|e| fail(e)); 
            }

            if let Some(at_least) = matches.value_of("at_least")
            {
                task.at_least = at_least.parse().unwrap();
            }
        }

        ("remove", Some(matches)) =>
        {
            let name = matches.value_of("name").unwrap();
            let index = schedule.tasks.iter().position(|t| t.name == name).or_fail("No task with that name");
            schedule.tasks.swap_remove(index);
            write_file(schedule_file, &schedule);
        }

        ("did", Some(matches)) =>
        {
            let name = matches.value_of("task").unwrap();

            let date = parse_date_or_today(matches.value_of("on"));
            let yes = matches.is_present("yes");

            let task_name = close_enough::close_enough(schedule.tasks.iter().map(|t| &t.name), name).or_fail("No task matching that name").to_owned();

            let (should_write, should_delete) = {
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
                    let date_completed = date;
                    let previous_date_due = task.date_due.as_naive().or_fail("Failed to parse date");
                    let repeat_start = if task.at_least { date_completed } else { previous_date_due };
                    let next_due_date = doq::next_due_date(repeat_start, date_completed, task.repeat);
                    let should_delete = match next_due_date
                    {
                        Some(next_due_date) => {
                            task.date_completed = Some(date_completed.into());
                            task.date_due = next_due_date.into();
                            false
                        },
                        None => true
                    };

                    (true, should_delete)
                }
                else
                {
                    eprintln!("Cancelling");
                    (false, false)
                }
            };

            if should_delete
            {
                // TODO: Remove DRY fail with remove command.
                let index = schedule.tasks.iter().position(|t| t.name == task_name).unwrap();
                schedule.tasks.swap_remove(index);
            }

            if should_write
            {
                write_file(schedule_file, &schedule);
            }
        },
        _ => ()
    }

    {
        // TODO: Stretch column sizes to fit max item
        println!("{: <20}      {: <33} {: <33}", "Task", "Last completed", "Due on");
        println!("{: <20}      {: <33} {: <33}", "===", "===", "===");

        let mut delta_tasks: Vec<_> = schedule.tasks.iter().map(
            |task| 
            {
                let date_due = task.date_due.as_naive().or_fail("Failed to parse date");
                let today = Utc::today().naive_utc();
                let delta = doq::days_until_due(date_due, today);
                (delta, task)
            }).collect();
        delta_tasks.sort_by_key(|&(delta, _)| delta);

        let red = Color::Fixed(9);
        let green = Color::Fixed(10);
        let yellow = Color::Fixed(11);

        for &(delta, task) in &delta_tasks
        {
            let leader = if task.at_least { '<' } else { ' ' };

            let freq_string = match task.repeat
            {
                Repeat::Days(days) => format!("{}{}d", leader, days),
                Repeat::Months(months) => format!("{}{}m", leader, months),
                Repeat::Years(years) => format!("{}{}y", leader, years),
                Repeat::Never => "--".to_owned()
            };

            let (datestring, days_ago_text) = match &task.date_completed
            {
                &Some(ref date) =>
                {
                    let date = date.as_naive().or_fail("Failed to parse date");
                    let days = Utc::today().naive_utc().signed_duration_since(date).num_days();
                    let days_ago_text = match days
                    {
                        0 => "    Today".to_owned(),
                        1 => "  1 day ago".to_owned(),
                        n => format!("{: >3} days ago", n)
                    };

                    (date.to_string(), days_ago_text)
                },
                &None => ("Never".to_owned(), "".to_owned())
            };

            let due_date_string = task.date_due.as_naive().or_fail("Failed to parse date").to_string();

            let (color, status) = match delta
            {
                delta if delta > 0 => (green, format!("(Due in {} days)", delta)),
                delta if delta == 0 => (yellow, format!("(Due today)")),
                delta => (red, format!("({} days overdue!)", -delta))
            };

            let line = format!("{: <20} {: >3}  {: <16} {: <16} {: <16} {: <16}", task.name, freq_string, datestring, days_ago_text, due_date_string, status);

            println!("{}", color.paint(line));
        }
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


fn parse_date(date: &str) -> NaiveDate
{
    use std::str::FromStr;
    NaiveDate::from_str(date).or_fail("Invalid date format")
}

fn parse_date_or_today(date: Option<&str>) -> NaiveDate
{
    match date
    {
        Some(date) => parse_date(date),
        None => Utc::today().naive_utc()
    }
}

