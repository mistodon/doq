extern crate chrono;

#[macro_use]
extern crate serde_derive;


pub mod data
{
    pub use chrono::{ Utc, NaiveDate, Duration };

    #[derive(Debug, Deserialize)]
    pub struct VersionedSchedule
    {
        pub tasks: Vec<VersionedTask>
    }

    #[derive(Debug, Default, Serialize)]
    pub struct Schedule
    {
        pub tasks: Vec<Task>
    }

    #[derive(Debug, Deserialize)]
    pub struct Task010
    {
        pub name: String,
        pub frequency_days: u32,
        pub last_completed: Option<Date>
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Task
    {
        pub name: String,
        pub date_completed: Option<Date>,
        pub date_due: Date,
        pub repeat: Repeat,
        pub at_least: bool
    }

    #[derive(Debug, Deserialize)]
    #[serde(untagged)]
    pub enum VersionedTask
    {
        Current(Task),
        Version010(Task010)
    }

    impl VersionedTask
    {
        pub fn upversioned(self) -> Option<Task>
        {
            match self
            {
                VersionedTask::Current(t) => Some(t),
                VersionedTask::Version010(t) => {
                    let Task010 { name, frequency_days, last_completed } = t;
                    let repeat = Repeat::Days(frequency_days);
                    let (date_completed, date_due) = if let Some(last_completed) = last_completed
                    {
                        let completed = last_completed.as_naive();

                        let date_due = match completed
                        {
                            Some(date) => super::next_due_date(date, date, repeat),
                            None => None
                        };

                        let date_due = match date_due
                        {
                            Some(date) => date,
                            None => return None
                        };

                        (completed.map(Into::into), date_due.into())
                    }
                    else
                    {
                        (None, Utc::today().naive_utc().into())
                    };

                    Some(Task
                    {
                        name,
                        date_completed,
                        date_due,
                        repeat,
                        at_least: false
                    })
                }
            }
        }
    }

    #[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub enum Repeat
    {
        Never,
        Days(u32),
        Months(u32),
        Years(u32)
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Date(String);

    impl From<NaiveDate> for Date
    {
        fn from(date: NaiveDate) -> Date
        {
            Date(format!("{}", date))
        }
    }

    impl Date
    {
        pub fn as_naive(&self) -> Option<NaiveDate>
        {
            use std::str::FromStr;

            NaiveDate::from_str(&self.0).ok()
        }
    }
}


use data::*;


pub fn repeat_from_string(string: &str) -> Result<Repeat, &'static str>
{
    const PARSE_ERROR: &str = "Expected a number";
    const UNIT_ERROR: &str = "Expected a suffix (d, m, y) for days, months, or years";

    if string == "never"
    {
        return Ok(Repeat::Never);
    }

    let (count, unit) = string.split_at(string.len() - 1);
    let count: u32 = match count.parse()
    {
        Ok(c) => c,
        Err(_) => return Err(PARSE_ERROR)
    };

    let repeat = match unit
    {
        "d" => Repeat::Days(count),
        "m" => Repeat::Months(count),
        "y" => Repeat::Years(count),
        _ => return Err(UNIT_ERROR)
    };

    Ok(repeat)
}

pub fn days_until_due(due_date: NaiveDate, today: NaiveDate) -> i64
{
    due_date.signed_duration_since(today).num_days() as i64
}


pub fn next_due_date(previous_date_due: NaiveDate, date_completed: NaiveDate, repeat: Repeat) -> Option<NaiveDate>
{
    use chrono::Datelike;
    use Repeat::*;

    let mut due_date = previous_date_due;

    while due_date <= date_completed
    {
        due_date = match repeat
        {
            Never => return None,
            Days(i) => due_date + Duration::days(i as i64),
            Months(i) => {
                let months = due_date.month0() + i;
                due_date
                    .with_year(due_date.year() + months as i32 / 12)
                    .and_then(|d| d.with_month0(months % 12))
                    .expect("TODO: something???")
            },
            Years(i) => due_date.with_year(due_date.year() + i as i32).expect("TODO: something?"),
        };
    }

    Some(due_date)
}


#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn test_days_until_due()
    {
        fn test(due: (i32, u32, u32), today: (i32, u32, u32)) -> i64
        {
            let (dy, dm, dd) = due;
            let (ty, tm, td) = today;
            days_until_due(NaiveDate::from_ymd(dy, dm, dd), NaiveDate::from_ymd(ty, tm, td))
        }

        assert_eq!(test((2017, 05, 27), (2017, 05, 27)), 0);
        assert_eq!(test((2017, 05, 27), (2017, 05, 26)), 1);
        assert_eq!(test((2017, 05, 27), (2017, 05, 28)), -1);
        assert_eq!(test((2017, 04, 27), (2017, 05, 27)), -30);
        assert_eq!(test((2017, 05, 27), (2017, 06, 27)), -31);
        assert_eq!(test((2017, 05, 27), (2016, 05, 27)), 365);
        assert_eq!(test((2016, 05, 27), (2015, 05, 27)), 366);
    }


    #[test]
    fn test_next_due_date()
    {
        fn test(due: (i32, u32, u32), completed: (i32, u32, u32), repeat: Repeat) -> Option<NaiveDate>
        {
            let (dy, dm, dd) = due;
            let (ty, tm, td) = completed;
            next_due_date(NaiveDate::from_ymd(dy, dm, dd), NaiveDate::from_ymd(ty, tm, td), repeat)
        }

        use Repeat::*;


        // Completed on due date
        assert_eq!(test((2017, 05, 27), (2017, 05, 27), Never), None);

        assert_eq!(test((2017, 05, 27), (2017, 05, 27), Days(1)),
            Some(NaiveDate::from_ymd(2017, 05, 28)));

        assert_eq!(test((2017, 05, 27), (2017, 05, 27), Days(5)),
            Some(NaiveDate::from_ymd(2017, 06, 01)));

        assert_eq!(test((2017, 05, 27), (2017, 05, 27), Days(365)),
            Some(NaiveDate::from_ymd(2018, 05, 27)));

        assert_eq!(test((2017, 05, 27), (2017, 05, 27), Months(1)),
            Some(NaiveDate::from_ymd(2017, 06, 27)));

        assert_eq!(test((2017, 05, 27), (2017, 05, 27), Months(12)),
            Some(NaiveDate::from_ymd(2018, 05, 27)));

        assert_eq!(test((2017, 05, 27), (2017, 05, 27), Months(14)),
            Some(NaiveDate::from_ymd(2018, 07, 27)));

        assert_eq!(test((2017, 05, 27), (2017, 05, 27), Years(1)),
            Some(NaiveDate::from_ymd(2018, 05, 27)));

        assert_eq!(test((2017, 05, 27), (2017, 05, 27), Years(7)),
            Some(NaiveDate::from_ymd(2024, 05, 27)));


        // Completed late
        assert_eq!(test((2017, 05, 27), (2017, 05, 30), Days(1)),
            Some(NaiveDate::from_ymd(2017, 05, 31)));

        assert_eq!(test((2017, 05, 27), (2017, 05, 30), Days(2)),
            Some(NaiveDate::from_ymd(2017, 05, 31)));

        assert_eq!(test((2017, 05, 27), (2017, 05, 30), Days(3)),
            Some(NaiveDate::from_ymd(2017, 06, 02)));

        assert_eq!(test((2017, 05, 27), (2017, 05, 31), Months(1)),
            Some(NaiveDate::from_ymd(2017, 06, 27)));

        assert_eq!(test((2017, 05, 27), (2017, 08, 31), Months(1)),
            Some(NaiveDate::from_ymd(2017, 09, 27)));

        assert_eq!(test((2017, 05, 27), (2017, 12, 12), Years(1)),
            Some(NaiveDate::from_ymd(2018, 05, 27)));

        assert_eq!(test((2017, 05, 27), (2020, 12, 12), Years(1)),
            Some(NaiveDate::from_ymd(2021, 05, 27)));


        // Completed early
        assert_eq!(test((2017, 05, 30), (2017, 05, 27), Days(1)),
            Some(NaiveDate::from_ymd(2017, 05, 30)));
    }
}
