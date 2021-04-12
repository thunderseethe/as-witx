use witx::{Bindgen, HandleDatatype, Id, IntRepr, InterfaceFunc, InterfaceFuncParam, Layout, Module, RecordDatatype, Type, TypeRef};
use itertools::Itertools;
use heck::*;

use crate::astype::*;
use crate::error::*;
use crate::pretty_writer::PrettyWriter;
use std::path::Path;

pub struct Generator {
    w: PrettyWriter,
    embed_header: bool,
}

trait Language {
    fn indent_bytes() -> &'static str {
        "    "
    }
    fn pretty_writer() -> PrettyWriter {
        let indent = <Self as Language>::indent_bytes();
        PrettyWriter::with_indent(indent)
    }
}
struct AssemblyScript<'a> {
    w: &'a mut PrettyWriter,
    params: &'a [InterfaceFuncParam],
    block_storage: Vec<String>,
    blocks: Vec<String>,
}

impl Language for AssemblyScript<'_> {}
impl Bindgen for AssemblyScript<'_> {
    type Operand = String;

    fn allocate_space(&mut self, slot: usize, ty: &witx::NamedType) {
        self.w.write_line(&format!("let rp{} = new {}();", slot, ty.name.as_str().to_camel_case()));
    }

    fn push_block(&mut self) {
        let mut prev = std::mem::replace(self.w, AssemblyScript::pretty_writer());
        self.block_storage.push(prev.finish());
    }

    fn finish_block(&mut self, operand: Option<Self::Operand>) {
        let to_restore = self.block_storage.pop().unwrap();
        let mut w = std::mem::replace(self.w, PrettyWriter::new(AssemblyScript::indent_bytes(), to_restore));
        let src = w.finish();
        match operand {
            None => {
                self.blocks.push("return;".to_string())
            },
            Some(s) => {
                if src.is_empty() {
                    self.blocks.push(format!("{}", s));
                } else {
                    // TODO: Figure out how to make sense of this in AS
                    // Probably need a closure that is immediately evaluated
                    self.blocks.push(format!("{{ {}; {} }}", src, s));
                }
            }
        }
    }

    fn emit(
        &mut self,
        inst: &witx::Instruction<'_>,
        operands: &mut Vec<Self::Operand>,
        results: &mut Vec<Self::Operand>,
    ) {
        use witx::Instruction;
        let mut top_as = |cvt: &str| {
            let mut s = operands.pop().unwrap();
            s.push_str(" as ");
            s.push_str(cvt);
            results.push(s);
        };

        match inst {
            Instruction::GetArg { nth } => {
                let res = self.params[*nth].name.render_to_string();
                results.push(res);
            },
            Instruction::AddrOf => {
                results.push(format!("changetype<usize>({})", operands[0]));
            }
            Instruction::I64FromBitflags { .. } | Instruction::I64FromU64 => top_as("i64"),
            Instruction::I32FromPointer
            | Instruction::I32FromConstPointer
            | Instruction::I32FromHandle { .. }
            | Instruction::I32FromUsize
            | Instruction::I32FromChar
            | Instruction::I32FromU8
            | Instruction::I32FromS8
            | Instruction::I32FromChar8
            | Instruction::I32FromU16
            | Instruction::I32FromS16
            | Instruction::I32FromU32
            | Instruction::I32FromBitflags { .. } => top_as("i32"),

            Instruction::EnumLower { .. } => {
                results.push(format!("{}.tag as i32", operands[0]))
            }
            Instruction::EnumLift { ty } => {
                let result = format!(
                    "new {}({} as {})",
                    ty.name.as_str().to_camel_case(),
                    &operands[0],
                    match ty.type_().as_ref() {
                        Type::Variant(v) => {
                            v.tag_repr.render_to_string()
                        }
                        _ => unreachable!()
                    }
                );
                results.push(result);
            }

            Instruction::F32FromIf32
            | Instruction::F64FromIf64
            | Instruction::If32FromF32
            | Instruction::If64FromF64
            | Instruction::I64FromS64
            | Instruction::I32FromS32 => {
                results.push(operands.pop().unwrap());
            }

            Instruction::ListPointerLength => {
                let list = operands.pop().unwrap();
                results.push(format!("changetype<usize>({})", list));
                results.push(format!("{}.length", list));
            }

            Instruction::S8FromI32 => top_as("i8"),
            Instruction::Char8FromI32 | Instruction::U8FromI32 => top_as("u8"),
            Instruction::S16FromI32 => top_as("i16"),
            Instruction::U16FromI32 => top_as("u16"),
            Instruction::S32FromI32 => {}
            Instruction::U32FromI32 => top_as("u32"),
            Instruction::S64FromI64 => {}
            Instruction::U64FromI64 => top_as("u64"),
            Instruction::UsizeFromI32 => top_as("usize"),
            Instruction::HandleFromI32 { .. } => top_as("u32"),
            Instruction::PointerFromI32 { .. } => top_as("*mut _"),
            Instruction::ConstPointerFromI32 { .. } => top_as("*const _"),
            Instruction::BitflagsFromI32 { .. } => unimplemented!(),
            Instruction::BitflagsFromI64 { .. } => unimplemented!(),

            Instruction::ReturnPointerGet { n } => {
                results.push(format!("changetype<usize>(rp{})", n));
            }

            Instruction::Load { ty } => {
                results.push(format!(
                    "load<{}>({})",
                    ty.name.as_str().to_camel_case(),
                    &operands[0],
                ));
            }

            Instruction::ReuseReturn => {
                results.push("ret".to_string());
            }

            // AssemblyScript doesn't support tuples yet
            Instruction::TupleLift { .. } => {
                unimplemented!()
            }
            Instruction::ResultLift { .. } => {
                let err = self.blocks.pop().unwrap();
                let ok = self.blocks.pop().unwrap();
                results.push(format!("{} == 0 ? WasiResult.ok({}) : WasiResult.err({})", operands[0], &ok, &err));
            }

            Instruction::CharFromI32 => unimplemented!(),

            Instruction::CallWasm {
                module,
                name,
                params: _,
                results: func_results,
            } => {
                assert!(func_results.len() < 2);
                if func_results.len() > 0 {
                    self.w.write("let ret = ");
                    results.push("ret".to_string());
                }
                self.w.write(&format!("{}.{}({});",
                    &module.to_snake_case(),
                    to_as_name(&name.to_snake_case()),
                    &operands.join(", ")
                ));
            }

            Instruction::Return { amt: 0 } => {}
            Instruction::Return { amt: 1 } => {
                self.w.write(&format!("return {};", &operands[0]));
            }
            // No tuple support
            Instruction::Return { .. } => unimplemented!(),

            Instruction::Store { .. }
            | Instruction::ListFromPointerLength { .. }
            | Instruction::CallInterface { .. }
            | Instruction::ResultLower { .. }
            | Instruction::TupleLower { .. }
            | Instruction::VariantPayload => unimplemented!(),
        }
    }
}

trait Render<T: Language> {
    fn render(&self, out: &mut PrettyWriter);

    fn render_to_string(&self) -> String {
        let mut w = T::pretty_writer();
        self.render(&mut w);
        w.finish()
    }
}

impl Render<AssemblyScript<'_>> for InterfaceFunc {
    fn render(&self, w: &mut PrettyWriter) {
        write_docs(w, &self.docs);
        /* Consider handling non snake case name */
        w.write("export declare function ");
        w.write(&to_as_name(&self.name
            .render_to_string()
            .to_snake_case()));
        
        let (params, results) = self.wasm_signature();
        assert!(results.len() <= 1);
        w.write("(");
        for (i, param) in params.iter().enumerate() {
            w.write(&format!("arg{}: ", i));
            param.render(w);
            w.write(", ");
        }
        w.write(")");

        if self.noreturn {
            w.write(": void");
        } else if let Some(result) = results.get(0) {
            w.write(": ");
            result.render(w);
        }
        w.write(";");
    }
}

impl Render<AssemblyScript<'_>> for witx::WasmType {
    fn render(&self, w: &mut PrettyWriter) {
        use witx::WasmType;
        match self {
            WasmType::F32 => w.write("f32"),
            WasmType::F64 => w.write("f64"),
            WasmType::I32 => w.write("i32"),
            WasmType::I64 => w.write("i64"),
        };
    }
}

impl Render<AssemblyScript<'_>> for Id {
    fn render(&self, w: &mut PrettyWriter) {
        w.write(&to_as_name(self.as_str()));
    }
}

impl Render<AssemblyScript<'_>> for IntRepr {
    fn render(&self, w: &mut PrettyWriter) {
        match self {
            IntRepr::U8 => w.write("u8"),
            IntRepr::U16 => w.write("u16"),
            IntRepr::U32 => w.write("u32"),
            IntRepr::U64 => w.write("u64"),
        };
    }
}

impl Render<AssemblyScript<'_>> for witx::TypeRef {
    fn render(&self, w: &mut PrettyWriter) {
        match self {
            TypeRef::Name(t) => {
                w.write(&t.name.as_str().to_camel_case());
            }
            TypeRef::Value(v) => {
                v.render(w);
            }
        }
    }
}

impl Render<AssemblyScript<'_>> for Module {
    fn render(&self, w: &mut PrettyWriter) {
        for f in self.funcs() {
            render_highlevel(&f, &self.name, w);
            w.write("\n\n");
        }
        // TODO: Finish module impl

        let as_name = self.name.as_str().to_camel_case();
        w.write("export namespace ")
         .write(&as_name)
         .braced(|w| {
            for f in self.funcs() {
                f.render(w);
                w.eob();
            }
         });
    }
}

fn render_highlevel(func: &InterfaceFunc, module: &Id, w: &mut PrettyWriter) {
    let as_name = func.name.render_to_string().to_snake_case();
    write_docs(w, &func.docs);
    // TODO: Write docs for params and results

    w.write("export function ")
     .write(&as_name)
     .write("(");

    

    w.write(&func.params.iter()
        .map(|p| {
            format!("{}: {}", p.name.render_to_string(), p.tref.render_to_string())
        })
        .intersperse(", ".to_string())
        .collect::<String>());

    w.write(")");

    match func.results.len() {
        0 => {},
        1 => {
            w.write(": ");
            func.results[0].tref.render(w);
        }
        _ => {
            // TODO: Figure out tuples
            unimplemented!()
        }
    }

    w.braced(|w| {
        let mut subw = AssemblyScript::pretty_writer();
        func.call_wasm(module, &mut AssemblyScript {
            w: &mut subw,
            params: &func.params,
            block_storage: Vec::new(),
            blocks: Vec::new(),
        });
        w.write(&subw.finish());
    });

}

impl Render<AssemblyScript<'_>> for witx::Type {
    fn render(&self, w: &mut PrettyWriter) {
        use witx::Type::*;
        match self {
            Builtin(builtin) => builtin.render(w),
            List(elem) => {
                w.write("StaticArray<");
                elem.render(w);
                w.write(">");
            }
            Variant(v) if v.is_bool() => { w.write("bool"); }
            Variant(v) => {
                match v.as_expected() {
                    None => panic!("reference to anonymous variant not supported"),
                    Some((ok, err)) => {
                        w.write("WasiResult<");
                        match ok {
                            Some(ty) => ty.render(w),
                            None => { w.write("void"); }
                        };
                        w.write(", ");
                        match err {
                            Some(ty) => ty.render(w),
                            None => { w.write("void"); }
                        }
                        w.write(">");
                    }
                }
            }
            Record(_) => panic!("reference to anonymous record not supported"),
            _ => unimplemented!(),
        };
    }
}

impl Render<AssemblyScript<'_>> for witx::BuiltinType {
    fn render(&self, w: &mut PrettyWriter) {
        use witx::BuiltinType::*;
        match self {
            U8 { lang_c_char: _ } => w.write("u8"),
            U16 => w.write("u16"),
            U32 { lang_ptr_size: false } => w.write("u32"),
            U32 { lang_ptr_size: true } => w.write("usize"),
            U64 => w.write("u64"),
            S8 => w.write("i8"),
            S16 => w.write("i16"),
            S32 => w.write("i32"),
            S64 => w.write("i64"),
            F32 => w.write("f32"),
            F64 => w.write("f64"),
            Char => w.write("u32"),
        };
    }
}

impl Render<AssemblyScript<'_>> for witx::NamedType {
    fn render(&self, w: &mut PrettyWriter) {
        let name = self.name.as_str();
        match &self.tref {
            TypeRef::Value(ty) => match ty.as_ref() {
                Type::Record(ref r) => render_record(w, name, r),
                Type::Handle(ref h) => render_handle(w, name, h),
                Type::Variant(ref v) => render_variant(w, name, v),
                Type::List {..}
                | Type::Pointer {..}
                | Type::ConstPointer {..}
                | Type::Builtin {..} => render_alias(w, name, &self.tref)
            }
            TypeRef::Name(_nt) => render_alias(w, name, &self.tref)
        }
    }
}

fn render_handle(w: &mut PrettyWriter, name: &str, _: &HandleDatatype) {
    w.write_line(&format!("export type {} = {};", name.to_camel_case(), ASType::Handle));
}

fn render_record(w: &mut PrettyWriter, name: &str, r: &RecordDatatype) {
    w.write_line("// @ts-ignore: decorator")
        .write_line("@unmanaged")
        .write(&format!("export class {} ", name.to_camel_case()));

    w.braced(|w| {
        for member in &r.members {
            let variant_name = to_as_name(member.name.as_str());
            let variant_type = ASType::from(&member.tref);
            write_docs(w, &member.docs);
            w.write_line(&format!("{}: {};", variant_name, variant_type));
        }
    });
}

fn render_alias(w: &mut PrettyWriter, name: &str, alias: &TypeRef) {
    w.write(&format!("export type {} = ", name.to_camel_case()));
    alias.render(w);
    w.write(";");
}

fn render_variant(
    w: &mut PrettyWriter,
    name: &str,
    union: &witx::Variant,
) {
    let as_tag = ASType::from(&union.tag_repr);
    let variants = &union.cases;
    let class_name = name.to_camel_case();

    let val_offset = union.payload_offset();
    let val_size = union.mem_size();
    w.write_line("// @ts-ignore: decorator")
        .write_line("@unmanaged")
        .write(&format!("export class {} ", class_name));
    w.braced(|w| {
        w.write_line(&format!("tag: {};", as_tag));
        let pad_len = val_offset + val_size;
        for i in 0..pad_len / 8 {
            w.write_line(&format!("private __pad64_{}: u64;", i));
        }
        for i in 0..(pad_len & 7) / 4 {
            w.write_line(&format!("private __pad32_{}: u32;", i));
        }
        for i in 0..(pad_len & 3) / 2 {
            w.write_line(&format!("private __pad16_{}: u16;", i));
        }
        for i in 0..(pad_len & 1) {
            w.write_line(&format!("private __pad8_{}: u8;", i));
        }
        w.eob();

        w.write_line(&format!("constructor(tag: {}) {{", as_tag));
        w.with_block(|w| {
            w.write_line("this.tag = tag;").write_line(&format!(
                "memory.fill(changetype<usize>(this) + {}, 0, {});",
                val_offset, val_size
            ));
        });
        w.write_line("}").eob();

        w.write_line("// @ts-ignore: default").write_line(&format!(
            "static new<T>(tag: u8, val: T = 0): {} {{",
            class_name
        ));
        w.with_block(|w| {
            w.write_line(&format!("let tu = new {}(tag);", class_name))
                .write_line("tu.set(val);")
                .write_line("return tu;");
        });
        w.write_line("}").eob();

        w.write_line("get<T>(): T {");
        w.with_block(|w| {
            w.write_line("// @ts-ignore: cast")
                .write_line(&format!(
                    "let valBuf = changetype<usize>(this) + {};",
                    val_offset
                ))
                .write_line("if (isReference<T>()) {");
            w.with_block(|w| {
                w.write_line("return changetype<T>(valBuf);"); });
            w.write_line("} else {");
            w.with_block(|w| {
                w.write_line("return load<T>(valBuf);"); });
            w.write_line("}");
        });
        w.write_line("}").eob();

        w.write_line("// @ts-ignore: default")
            .write_line("set<T>(val: T = 0): void {");
        w.with_block(|w| {
            w.write_line("// @ts-ignore: cast")
                .write_line(&format!(
                    "let valBuf = changetype<usize>(this) + {};",
                    val_offset
                ))
                .write_line(&format!("memory.fill(valBuf, 0, {});", val_size))
                .write_line("if (isReference<T>()) {");
            w.with_block(|w| {
                w.write_line("(val !== null) && memory.copy(valBuf, changetype<usize>(val), offsetof<T>());");
            });
            w.write_line("} else {");
            w.with_block(|w| { w.write_line("store<T>(valBuf, val)"); });
            w.write_line("}");
        });
        w.write_line("}");

        for (i, variant) in variants.iter().enumerate() {
            w.eob();
            define_variant_case(w, &class_name, i, variant);
        }
    });
}

impl Generator {
    pub fn new(module_name: Option<String>, embed_header: bool) -> Self {
        let w = PrettyWriter::with_indent("    ");
        Generator { w, embed_header }
    }

    pub fn generate<P: AsRef<Path>>(&mut self, path: P) -> Result<String, Error> {
        let document = witx::load(&[path])?;
        if self.embed_header {
            self.header();
        }
        for type_ in document.typenames() {
            //self.define_type(type_.as_ref());
            type_.render(&mut self.w);
        }
        for module in document.modules() {
            module.render(&mut self.w);
        }
        for c in document.constants() {
            self.w.write_line(&format!("public const {}_{}: {} = {};"
                , c.ty.as_str().to_shouty_snake_case()
                , c.name.as_str().to_shouty_snake_case()
                , c.ty.as_str().to_camel_case()
                , c.value
                ));
        }
        Ok(self.w.finish())
    }

    fn header(&mut self) {
        self.w.write_lines(
            "
/*
 * This file was automatically generated by as-witx - Do not edit manually.
 */",
        );
        self.w.write_lines(
            "
export type handle = i32;
export type char = u8;
export type ptr<T> = usize;
export type mut_ptr<T> = usize;
export type untyped_ptr = usize;
export type struct<T> = usize;
export type union<T> = usize;
export type wasi_string_ptr = ptr<char>;
",
        );
        self.w.write_lines(
            "
@unmanaged
export class WasiString {
    ptr: wasi_string_ptr;
    length: usize;

    constructor(str: string) {
        let wasiString = String.UTF8.encode(str, false);
        // @ts-ignore: cast
        this.ptr = changetype<wasi_string_ptr>(wasiString);
        this.length = wasiString.byteLength;
    }

    toString(): string {
        let tmp = new ArrayBuffer(this.length as u32);
        memory.copy(changetype<usize>(tmp), this.ptr, this.length);
        return String.UTF8.decode(tmp);
    }
}

@unmanaged
export class WasiArray<T> {
    ptr: ptr<T>;
    length: usize;

    constructor(array: ArrayBufferView) {
        // @ts-ignore: cast
        this.ptr = array.dataStart;
        this.length = array.byteLength;
    }
}
",
        )
        .eob();
    }
}

fn define_variant_case(
    w: &mut PrettyWriter,
    class_name: &str,
    i: usize,
    variant: &witx::Case,
) {
    let variant_name = variant.name.as_str();
    match variant.tref.as_ref() {
        None => {
            w.write_line(&format!("// --- {}: void if tag={}", variant_name, i));
        }
        Some(variant_type) => {
            w.write_line(&format!(
                "// --- {}: {} if tag={}",
                variant_name,
                ASType::from(variant_type),
                i
            ));
        }
    }
    w.eob();
    define_variant_case_accessors(w, class_name, i, variant);
}

fn define_variant_case_accessors(
    w: &mut PrettyWriter,
    class_name: &str,
    i: usize,
    variant: &witx::Case,
) {
    let variant_name = variant.name.as_str();
    match variant.tref.as_ref() {
        None => {
            w.write_line(&format!("static {}(): {} {{", variant_name, class_name))
                .indent()
                .write_line(&format!("return {}.new({});", class_name, i))
                .write_line("}")
                .eob();

            w.write_line(&format!("set_{}(): void {{", variant_name))
                .indent()
                .write_line(&format!("this.tag = {};", i))
                .write_line("}")
                .eob();

            w.write_line(&format!("is_{}(): bool {{", variant_name))
                .indent()
                .write_line(&format!("return this.tag === {};", i))
                .write_line("}");
        }
        Some(variant_type) => {
            let as_variant_type = ASType::from(variant_type);
            w.write_line(&format!(
                "static {}(val: {}): {} {{",
                variant_name, as_variant_type, class_name
            ));
            w.with_block(|w| {
                w.write_line(&format!("return {}.new({}, val);", class_name, i));
            });
            w.write_line("}").eob();

            w.write_line(&format!(
                "set_{}(val: {}): void {{",
                variant_name, as_variant_type
            ));
            w.with_block(|w| {
                w.write_line(&format!("this.tag = {};", i))
                    .write_line("this.set(val);");
            });
            w.write_line("}").eob();

            w.write_line(&format!("is_{}(): bool {{", variant_name))
                .indent()
                .write_line(&format!("return this.tag === {};", i))
                .write_line("}")
                .eob();

            if as_variant_type.is_nullable() {
                w.write_line(&format!(
                    "get_{}(): {} | null {{",
                    variant_name, as_variant_type
                ));
            } else {
                w.write_line(&format!("get_{}(): {} {{", variant_name, as_variant_type));
            }
            w.with_block(|w| {
                if as_variant_type.is_nullable() {
                    w.write_line(&format!("if (this.tag !== {}) {{ return null; }}", i));
                }
                w.write_line(&format!("return this.get<{}>();", as_variant_type));
            });
            w.write_line("}");
        }
    }
}

fn write_docs(w: &mut PrettyWriter, docs: &str) {
    if docs.is_empty() {
        return;
    }
    w.write_line("/**");
    for docs_line in docs.lines() {
        w.write_line(&format!(" * {}", docs_line));
    }
    w.write_line(" */");
}

fn to_as_name(name: &str) -> String {
    if let Ok(_) = name.parse::<usize>() {
        format!("_{}", name)
    } else {
        name.to_owned()
    }

}
