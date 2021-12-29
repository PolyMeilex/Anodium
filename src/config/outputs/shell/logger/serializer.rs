use slog::Key;
use std::fmt;
use std::fmt::Write;

pub struct ShellSerializer {
    data: String,
}

impl ShellSerializer {
    pub fn start(len: Option<usize>) -> Result<Self, slog::Error> {
        let data = String::with_capacity(len.unwrap_or(120));

        Ok(ShellSerializer { data })
    }

    pub fn tag_serializer(&mut self) -> ShellSocketTagSerializer {
        ShellSocketTagSerializer {
            data: &mut self.data,
        }
    }

    pub fn field_serializer(&mut self) -> ShellSocketFieldSerializer {
        ShellSocketFieldSerializer {
            data: &mut self.data,
        }
    }

    pub fn tag_value_break(&mut self) -> slog::Result {
        self.data.write_char(' ').map_err(|e| e.into())
    }

    pub fn end(self) -> Result<String, slog::Error> {
        let mut data = self.data;
        data.write_char('\n')?;
        Ok(data)
    }
}

pub struct ShellSocketTagSerializer<'a> {
    data: &'a mut String,
}

macro_rules! emit_m {
    ($f:ident, $arg:ty) => {
        fn $f(&mut self, key: Key, val: $arg) -> slog::Result {
            self.data
                .write_fmt(format_args!(" {}={} ", key, val))
                .map_err(|e| e.into())
        }
    };
}

impl<'a> slog::Serializer for ShellSocketTagSerializer<'a> {
    emit_m!(emit_u8, u8);
    emit_m!(emit_i8, i8);
    emit_m!(emit_u16, u16);
    emit_m!(emit_i16, i16);
    emit_m!(emit_usize, usize);
    emit_m!(emit_isize, isize);
    emit_m!(emit_u32, u32);
    emit_m!(emit_i32, i32);
    emit_m!(emit_u64, u64);
    emit_m!(emit_i64, i64);
    emit_m!(emit_f32, f32);
    emit_m!(emit_f64, f64);
    emit_m!(emit_bool, bool);
    emit_m!(emit_char, char);
    emit_m!(emit_str, &str);

    // Serialize '()' as '0'
    fn emit_unit(&mut self, key: Key) -> slog::Result {
        self.data
            .write_fmt(format_args!(",{}=0", key))
            .map_err(|e| e.into())
    }

    // Serialize 'None' as 'false'
    fn emit_none(&mut self, key: Key) -> slog::Result {
        self.data
            .write_fmt(format_args!(",{}=f", key))
            .map_err(|e| e.into())
    }

    emit_m!(emit_arguments, &fmt::Arguments);
}

pub struct ShellSocketFieldSerializer<'a> {
    data: &'a mut String,
}

impl<'a> ShellSocketFieldSerializer<'a> {
    fn write_int(&mut self, key: Key, integer: i64) -> slog::Result {
        self.data
            .write_fmt(format_args!("{}={}i", key, integer))
            .map_err(|e| e.into())
    }

    fn write_float(&mut self, key: Key, float: f64) -> slog::Result {
        self.data
            .write_fmt(format_args!("{}={}", key, float))
            .map_err(|e| e.into())
    }
}

impl<'a> slog::Serializer for ShellSocketFieldSerializer<'a> {
    fn emit_u8(&mut self, key: Key, val: u8) -> slog::Result {
        self.write_int(key, val as i64)
    }

    fn emit_i8(&mut self, key: Key, val: i8) -> slog::Result {
        self.write_int(key, val as i64)
    }

    fn emit_u16(&mut self, key: Key, val: u16) -> slog::Result {
        self.write_int(key, val as i64)
    }

    fn emit_i16(&mut self, key: Key, val: i16) -> slog::Result {
        self.write_int(key, val as i64)
    }

    fn emit_usize(&mut self, key: Key, val: usize) -> slog::Result {
        self.write_int(key, val as i64)
    }

    fn emit_isize(&mut self, key: Key, val: isize) -> slog::Result {
        self.write_int(key, val as i64)
    }

    fn emit_u32(&mut self, key: Key, val: u32) -> slog::Result {
        self.write_int(key, val as i64)
    }

    fn emit_i32(&mut self, key: Key, val: i32) -> slog::Result {
        self.write_int(key, val as i64)
    }

    fn emit_u64(&mut self, key: Key, val: u64) -> slog::Result {
        self.write_int(key, val as i64)
    }

    fn emit_i64(&mut self, key: Key, val: i64) -> slog::Result {
        self.write_int(key, val)
    }

    fn emit_f32(&mut self, key: Key, val: f32) -> slog::Result {
        self.write_float(key, val as f64)
    }

    fn emit_f64(&mut self, key: Key, val: f64) -> slog::Result {
        self.write_float(key, val)
    }

    fn emit_bool(&mut self, key: Key, val: bool) -> slog::Result {
        if val {
            self.data
                .write_fmt(format_args!("{}=t", key))
                .map_err(|e| e.into())
        } else {
            self.data
                .write_fmt(format_args!("{}=f", key))
                .map_err(|e| e.into())
        }
    }

    fn emit_char(&mut self, key: Key, val: char) -> slog::Result {
        self.data
            .write_fmt(format_args!(r#"{}="{}""#, key, val))
            .map_err(|e| e.into())
    }

    fn emit_str(&mut self, key: Key, val: &str) -> slog::Result {
        self.data
            .write_fmt(format_args!(r#"{}="{}""#, key, val))
            .map_err(|e| e.into())
    }

    // Serialize '()' as '0'
    fn emit_unit(&mut self, key: Key) -> slog::Result {
        self.data
            .write_fmt(format_args!("{}=0", key))
            .map_err(|e| e.into())
    }

    // Serialize 'None' as 'false'
    fn emit_none(&mut self, key: Key) -> slog::Result {
        self.data
            .write_fmt(format_args!("{}=f", key))
            .map_err(|e| e.into())
    }

    fn emit_arguments(&mut self, key: Key, val: &fmt::Arguments) -> slog::Result {
        self.data
            .write_fmt(format_args!("{}=\"{}\"", key, val))
            .map_err(|e| e.into())
    }
}
