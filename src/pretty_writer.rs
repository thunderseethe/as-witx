
#[derive(Clone)]
pub struct PrettyWriter {
    writer: String,
    indent: usize,
    indent_bytes: &'static str,
    continuation_bytes: &'static str,
}

const DEFAULT_CONTINUATION_BYTES: &str = "    ";

impl PrettyWriter {    
    /// Create a new `PrettyWriter` with `indent` initial units of indentation
    pub fn new_with_indent(indent: usize, indent_bytes: &'static str) -> Self {
        PrettyWriter {
            writer: String::new(),
            indent,
            indent_bytes,
            continuation_bytes: DEFAULT_CONTINUATION_BYTES,
        }
    }

    pub fn new(indent_bytes: &'static str) -> Self {
        Self::new_with_indent(0, indent_bytes)
    }
    /// Run block_ops as an indented block within the current `PrettyWriter`
    pub fn with_block(&mut self, block_ops: impl FnOnce(&mut Self) -> ()) -> &mut Self {
       self.indent += 1;
       block_ops(self);
       self.indent -= 1;
       self
    }

    pub fn braced(&mut self, braced_ops: impl FnOnce(&mut Self) -> ()) -> &mut Self {
        self.writer.push('{');
        self.with_block(braced_ops);
        self.writer.push('}');
        self
    }

    /// Write raw data
    pub fn write(&mut self, buf: &str) -> &mut Self {
        self.writer.push_str(buf);
        self
    }
    
    /// Indent, write raw data and terminate with an end of line
    pub fn write_line(&mut self, buf: &str) -> &mut Self {
        self.indent();
        self.write(buf);
        self.writer.push('\n');
        return self;
    }

    /// Write multiple indented lines
    pub fn write_lines(&mut self, buf: &str) -> &mut Self {
        for line in buf.lines() {
            self.write_line(line);
        }
        self
    }

    /// Output an indentation string
    pub fn indent(&mut self) -> &mut Self {
        self.writer.extend(
            std::iter::repeat(self.indent_bytes)
                    .take(self.indent)
                    .flat_map(|s| s.chars()));
        self
    }

    /// Continuation
    pub fn continuation(&mut self) -> &mut Self {
        self.indent();
        self.writer.push_str(self.continuation_bytes);
        self
    }

    pub fn eol(&mut self) -> &mut Self {
        self.writer.push('\n');
        self
    }

    pub fn eob(&mut self) -> &mut Self {
        self.eol()
    }

    pub fn finish(&mut self) -> String {
        self.writer.clone()
    }
}

impl std::fmt::Write for PrettyWriter {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.indent();
        self.writer.write_str(s)
    }
}

//impl<W: Write> PrettyWriter<W> {
//    /// Create a new `PrettyWriter` with `indent` initial units of indentation
//    pub fn new_with_indent(writer: W, indent: u32, indent_bytes: &'static str) -> Self {
//        PrettyWriter {
//            writer: Rc::new(RefCell::new(writer)),
//            indent,
//            indent_bytes,
//            continuation_bytes: DEFAULT_CONTINUATION_BYTES,
//        }
//    }
//
//    /// Create a new `PrettyWriter` with no initial indentation
//    pub fn new(writer: W, indent_bytes: &'static str) -> Self {
//        PrettyWriter::new_with_indent(writer, 0, indent_bytes)
//    }
//
//    /// Create a writer based on a existing writer, but with no indentation`
//    #[allow(dead_code)]
//    pub fn new_from_writer(&mut self) -> Self {
//        PrettyWriter {
//            writer: self.writer.clone(),
//            indent: 0,
//            indent_bytes: self.indent_bytes,
//            continuation_bytes: DEFAULT_CONTINUATION_BYTES,
//        }
//    }
//
//    /// Create an indented block within the current `PrettyWriter`
//    //pub fn new_block(&mut self) -> Self {
//    //    PrettyWriter {
//    //        writer: self.writer.clone(),
//    //        indent: self.indent + 1,
//    //        indent_bytes: self.indent_bytes,
//    //        continuation_bytes: DEFAULT_CONTINUATION_BYTES,
//    //    }
//    //}
//
//    /// Run block_ops as an indented block within the current `PrettyWriter`
//    pub fn with_block<'a: 'b, 'b>(&'a mut self, block_ops: impl FnOnce(&'b mut Self) -> Result<(), Error>) -> Result<&'a mut Self, Error> {
//       self.indent += 1;
//       block_ops(&mut self).map(|_| ())?;
//       self.indent -= 1;
//       return Ok(self);
//    }
//
//    fn _write_all<T: AsRef<[u8]>>(writer: &mut W, buf: T) -> Result<(), Error> {
//        let buf = buf.as_ref();
//        writer.write_all(buf).map_err(Into::into)
//    }
//
//    /// Return the current indentation level
//    #[allow(dead_code)]
//    pub fn indent_level(&self) -> u32 {
//        self.indent
//    }
//
//    /// Output an indentation string
//    pub fn indent(&mut self) -> Result<&mut Self, Error> {
//        let indent_bytes = &self.indent_bytes;
//        {
//            let mut writer = self.writer.borrow_mut();
//            for _ in 0..self.indent {
//                Self::_write_all(&mut writer, indent_bytes)?
//            }
//        }
//        Ok(self)
//    }
//
//    /// Output a space
//    #[allow(dead_code)]
//    pub fn space(&mut self) -> Result<&mut Self, Error> {
//        Self::_write_all(&mut self.writer.borrow_mut(), b" ")?;
//        Ok(self)
//    }
//
//    /// Output an end of line
//    pub fn eol(&mut self) -> Result<&mut Self, Error> {
//        Self::_write_all(&mut self.writer.borrow_mut(), b"\n")?;
//        Ok(self)
//    }
//
//    /// Output a block separator
//    pub fn eob(&mut self) -> Result<&mut Self, Error> {
//        self.eol()
//    }
//
//    /// Continuation
//    pub fn continuation(&mut self) -> Result<&mut Self, Error> {
//        self.indent()?;
//        let continuation_bytes = &self.continuation_bytes;
//        Self::_write_all(&mut self.writer.borrow_mut(), continuation_bytes)?;
//        Ok(self)
//    }
//
//    /// Write raw data
//    pub fn write<T: AsRef<[u8]>>(&mut self, buf: T) -> Result<&mut Self, Error> {
//        let buf = buf.as_ref();
//        Self::_write_all(&mut self.writer.borrow_mut(), buf)?;
//        Ok(self)
//    }
//
//    /// Indent, write raw data and terminate with an end of line
//    pub fn write_line<T: AsRef<[u8]>>(&mut self, buf: T) -> Result<&mut Self, Error> {
//        let buf = buf.as_ref();
//        self.indent()?.write(buf)?.eol()
//    }
//
//    /// Write multiple indented lines
//    pub fn write_lines<T: AsRef<[u8]>>(&mut self, buf: T) -> Result<&mut Self, Error> {
//        let buf = buf.as_ref();
//        for line in buf.lines() {
//            if let Ok(line) = line {
//                self.write_line(line)?;
//            }
//        }
//        Ok(self)
//    }
//}
