use slog::FilterLevel;
use std::fmt;

pub struct Filter {
    inner: String,
}

impl Filter {
    pub fn new(spec: &str) -> Result<Filter, String> {
        Ok(Filter {
            inner: spec.to_string(),
        })
    }

    pub fn is_match(&self, s: &str) -> bool {
        s.contains(&self.inner)
    }
}

impl fmt::Display for Filter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

pub struct LogDirective {
    pub name: Option<String>,
    pub level: FilterLevel,
}

pub fn parse_logging_spec(spec: &str) -> (Vec<LogDirective>, Option<Filter>) {
    let mut dirs = Vec::new();

    let mut parts = spec.split('/');
    let mods = parts.next();
    let filter = parts.next();
    if parts.next().is_some() {
        println!(
            "warning: invalid logging spec '{}', \
                 ignoring it (too many '/'s)",
            spec
        );
        return (dirs, None);
    }
    if let Some(m) = mods {
        for s in m.split(',') {
            if s.is_empty() {
                continue;
            }
            let mut parts = s.split('=');
            let (log_level, name) =
                match (parts.next(), parts.next().map(|s| s.trim()), parts.next()) {
                    (Some(part0), None, None) => {
                        // if the single argument is a log-level string or number,
                        // treat that as a global fallback
                        match part0.parse() {
                            Ok(num) => (num, None),
                            Err(_) => (FilterLevel::max(), Some(part0)),
                        }
                    }
                    (Some(part0), Some(""), None) => (FilterLevel::max(), Some(part0)),
                    (Some(part0), Some(part1), None) => match part1.parse() {
                        Ok(num) => (num, Some(part0)),
                        _ => {
                            println!(
                                "warning: invalid logging spec '{}', \
                                 ignoring it",
                                part1
                            );
                            continue;
                        }
                    },
                    _ => {
                        println!(
                            "warning: invalid logging spec '{}', \
                         ignoring it",
                            s
                        );
                        continue;
                    }
                };
            dirs.push(LogDirective {
                name: name.map(|s| s.to_string()),
                level: log_level,
            });
        }
    }

    let filter = filter.and_then(|filter| match Filter::new(filter) {
        Ok(re) => Some(re),
        Err(e) => {
            println!("warning: invalid regex filter - {}", e);
            None
        }
    });

    (dirs, filter)
}
