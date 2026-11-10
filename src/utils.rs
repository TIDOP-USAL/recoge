use chrono::{DateTime, Local, Utc};

pub fn get_date_local(date: &DateTime<Utc>) -> String {
    let date: DateTime<Local> = (*date).into();
    let formatted_date = date.format("%d/%m/%Y %H:%M").to_string();
    formatted_date
}

pub fn get_date_local_seconds(date: &DateTime<Utc>) -> String {
    let date: DateTime<Local> = (*date).into();
    let formatted_date = date.format("%d/%m/%Y %H:%M:%S").to_string();
    formatted_date
}

pub fn get_relative_date(date: &DateTime<Utc>) -> String {
    let duration = chrono::Utc::now() - date;
    let dur_secs = duration.num_seconds();
    if dur_secs < 0 {
        return String::new();
    }
    let dur_mins = f64::round(dur_secs as f64 / 60.0) as u64;
    if dur_mins == 0 {
        return format!("{dur_secs}s");
    }
    if dur_mins < 60 {
        return format!("{dur_mins}min");
    }
    let dur_hours = f64::round(dur_mins as f64 / 60.0) as u64;
    if dur_hours < 24 {
        return format!("{dur_hours}h");
    }
    let dur_days = f64::round(dur_hours as f64 / 24.0) as u64;
    if dur_days < 30 {
        return format!("{dur_days}d");
    }
    let dur_months = f64::round(dur_days as f64 / 30.0) as u64;
    if dur_months < 12 {
        return format!("{dur_months}M");
    }
    let dur_years = f64::round(dur_days as f64 / 365.25) as u64;
    format!("{dur_years}y")
}

pub fn get_relative_date_long(date: &DateTime<Utc>) -> String {
    let duration = chrono::Utc::now() - date;
    let dur_secs = duration.num_seconds();
    if dur_secs < 0 {
        return String::new();
    }
    let dur_mins = f64::round(dur_secs as f64 / 60.0) as u64;
    if dur_mins == 0 {
        return format!("{dur_secs} segundos");
    }
    if dur_mins < 60 {
        if dur_mins == 1 {
            return format!("{dur_mins} minuto");
        } else {
            return format!("{dur_mins} minutos");
        }
    }
    let dur_hours = f64::round(dur_mins as f64 / 60.0) as u64;
    if dur_hours < 24 {
        if dur_hours == 1 {
            return format!("{dur_hours} hora");
        } else {
            return format!("{dur_hours} horas");
        }
    }
    let dur_days = f64::round(dur_hours as f64 / 24.0) as u64;
    if dur_days < 30 {
        if dur_days == 1 {
            return format!("{dur_days} día");
        } else {
            return format!("{dur_days} días");
        }
    }
    let dur_months = f64::round(dur_days as f64 / 30.0) as u64;
    if dur_months < 12 {
        if dur_months == 1 {
            return format!("{dur_months} mes");
        } else {
            return format!("{dur_months} meses");
        }
    }
    let dur_years = f64::round(dur_days as f64 / 365.25) as u64;
    if dur_years == 1 {
        format!("{dur_years} año")
    } else {
        format!("{dur_years} años")
    }
}
