use slog::{o, Record};
use slog::{FnValue, PushFnValue};
use slog::{Level, KV};
use slog::{OwnedKVList, SendSyncRefUnwindSafeKV};

use super::filter::{parse_logging_spec, Filter, LogDirective};
use super::serializer::ShellSerializer;

use std::collections::VecDeque;
use std::io;
use std::sync::Mutex;

use lazy_static::lazy_static;

const BUFFER_SIZE: usize = 32;

pub struct ShellBuffer {
    pub buffer: VecDeque<(Level, String)>,
    pub updated: bool,
}

impl ShellBuffer {
    pub fn new() -> Self {
        Self {
            buffer: VecDeque::with_capacity(BUFFER_SIZE),
            updated: true,
        }
    }
}

lazy_static! {
    pub static ref BUFFER: Mutex<ShellBuffer> = Mutex::new(ShellBuffer::new());
}

pub struct ShellDrain {
    values: Vec<OwnedKVList>,
    filter: Option<Filter>,
    directives: Vec<LogDirective>,
}

impl ShellDrain {
    pub fn new() -> ShellDrain {
        ShellDrainBuilder::new()
            .default_tags()
            .build()
            .parse(&std::env::var("RUST_LOG").unwrap_or_else(|_| String::new()))
    }

    pub fn parse(mut self, filters: &str) -> Self {
        let (directives, filter) = parse_logging_spec(filters);

        self.filter = filter;

        for directive in directives {
            self.directives.push(directive);
        }
        self
    }

    fn enabled(&self, level: Level, module: &str) -> bool {
        // Search for the longest match, the vector is assumed to be pre-sorted.
        for directive in self.directives.iter().rev() {
            match directive.name {
                Some(ref name) if !module.starts_with(&**name) => {}
                Some(..) | None => return level.as_usize() <= directive.level.as_usize(),
            }
        }
        false
    }
}

impl slog::Drain for ShellDrain {
    type Ok = ();
    type Err = io::Error;

    fn log(&self, info: &Record, val: &OwnedKVList) -> io::Result<()> {
        if !self.enabled(info.level(), info.module()) {
            return Ok(());
        }

        if let Some(filter) = self.filter.as_ref() {
            if !filter.is_match(&format!("{}", info.msg())) {
                return Ok(());
            }
        }

        let mut serializer = ShellSerializer::start(None)?;
        let mut tag_serializer = serializer.tag_serializer();

        for kv in &self.values {
            kv.serialize(info, &mut tag_serializer)?;
        }

        val.serialize(info, &mut tag_serializer)?;
        serializer.tag_value_break()?;

        let mut field_serializer = serializer.field_serializer();
        info.kv().serialize(info, &mut field_serializer)?;

        let data = serializer.end()?;

        let data = format!("[{}] {}", chrono::Local::now().format("%H:%M:%S.%3f"), data);

        let mut buffer = BUFFER.lock().unwrap();

        buffer.buffer.push_front((info.level(), data));
        buffer.buffer.truncate(BUFFER_SIZE);
        buffer.updated = true;

        Ok(())
    }
}

pub struct ShellDrainBuilder {
    values: Vec<OwnedKVList>,
}

impl ShellDrainBuilder {
    pub fn new() -> Self {
        ShellDrainBuilder { values: vec![] }
    }

    pub fn build(self) -> ShellDrain {
        ShellDrain {
            values: self.values,
            filter: None,
            directives: vec![],
        }
    }

    pub fn add_tag_kv<T>(mut self, value: slog::OwnedKV<T>) -> Self
    where
        T: SendSyncRefUnwindSafeKV + 'static,
    {
        self.values.push(value.into());
        self
    }

    pub fn default_tags(self) -> Self {
        self.add_tag_kv(o!(
            "msg" => PushFnValue(move |record, ser| ser.emit(record.msg())),
            "mod" => FnValue(move |rinfo| rinfo.module()),

        ))
    }
}
