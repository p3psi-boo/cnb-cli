use anyhow::Result;
use serde::Serialize;
use std::io::{self, Write};

pub fn output_one<T>(value: &T, json: bool) -> Result<()>
where
    T: Serialize + std::fmt::Display,
{
    if json {
        output_json(value)
    } else {
        println!("{}", value);
        Ok(())
    }
}

pub fn output_list<T>(values: &[T], json: bool) -> Result<()>
where
    T: Serialize + std::fmt::Display,
{
    if json {
        output_json(values)
    } else {
        for value in values {
            println!("{}", value);
        }
        Ok(())
    }
}

pub fn output_created<T>(label: &str, value: &T, json: bool) -> Result<()>
where
    T: Serialize + std::fmt::Display,
{
    if json {
        output_json(value)
    } else {
        println!("{label}: {value}");
        Ok(())
    }
}

pub fn output_json<T: Serialize + ?Sized>(value: &T) -> Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    serde_json::to_writer_pretty(&mut handle, value)?;
    writeln!(handle)?;
    Ok(())
}
