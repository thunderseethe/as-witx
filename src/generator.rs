use witx::Layout;
use heck::*;

use crate::astype::*;
use crate::error::*;
use crate::pretty_writer::PrettyWriter;
use std::path::Path;

pub struct Generator {
    w: PrettyWriter,
    module_name: Option<String>,
    embed_header: bool,
}

impl Generator {
    pub fn new(module_name: Option<String>, embed_header: bool) -> Self {
        let w = PrettyWriter::new("    ");
        Generator { w, module_name, embed_header }
    }

    pub fn generate<P: AsRef<Path>>(&mut self, path: P) -> Result<String, Error> {
        let document = witx::load(&[path])?;
        if self.embed_header {
            self.header();
        }
        for type_ in document.typenames() {
            self.define_type(type_.as_ref());
        }
        for module in document.modules() {
            self.define_module(module.as_ref());
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

    fn define_as_alias(
        &mut self,
        as_type: &ASType,
        other_type: &ASType,
    ) {
        self.w.write_line(&format!("export type {} = {};", as_type, other_type));
    }

    fn define_as_handle(&mut self, as_type: &ASType) {
        self.w.write_line(&format!("export type {} = {};", as_type, ASType::Handle));
    }

    fn define_as_variant(
        &mut self,
        as_type: &ASType,
        union: &witx::Variant,
    ) {
        let as_tag = ASType::from(&union.tag_repr);
        let variants = &union.cases;

        let val_offset = union.payload_offset();
        let val_size = union.mem_size();
        self.w.write_line("// @ts-ignore: decorator")
         .write_line("@unmanaged")
         .write_line(&format!("export class {} {{", as_type));
        self.w.with_block(|w| {
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
                as_type
            ));
            w.with_block(|w| {
                w.write_line(&format!("let tu = new {}(tag);", as_type))
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
                define_variant_case(w, as_type, i, variant);
            }
        });
        self.w.write_line("}");
    }

    fn define_as_builtin(
        &mut self,
        as_type: &ASType,
        actual_as_type: &ASType,
    ) -> () {
        self.w.write_line(&format!("export type {} = {};", as_type, actual_as_type));
    }

    fn define_as_record(
        &mut self,
        as_type: &ASType,
        record: &witx::RecordDatatype,
    ) {
        self.w.write_line("// @ts-ignore: decorator")
            .write_line("@unmanaged")
            .write("export class ")
            .write(&(format!("{} ", as_type).to_camel_case()));
        self.w.braced(|w| {
            for member in &record.members {
                let variant_name = to_as_name(member.name.as_str());
                let variant_type = ASType::from(&member.tref);
                write_docs(w, &member.docs);
                w.write_line(&format!("{}: {};", variant_name, variant_type));
            }
        });
        self.w.write_line("}");
    }

    fn define_as_list(
        &mut self,
        as_type: &ASType,
        actual_as_type: &ASType,
    ) {
        self.w.write_line(&format!(
            "export type {} = WasiArray<{}>;",
            as_type, actual_as_type
        ));
    }

    fn define_as_witx_type(
        &mut self,
        as_type: &ASType,
        witx_type: &witx::Type,
    ) {
        use witx::Type::*;
        match witx_type {
            Handle(_handle) => self.define_as_handle(as_type),
            Builtin(builtin) => self.define_as_builtin(as_type, &builtin.into()),
            Variant(ref variant) => self.define_as_variant(as_type, variant),
            Record(ref record) =>  self.define_as_record(as_type, record),
            List(elem) => self.define_as_list(as_type, &ASType::from(elem)),
            ConstPointer(_) | witx::Type::Pointer(_) => {
                panic!("Typedef's pointers are not implemented")
            }
        };
    }

    fn define_type(&mut self, type_: &witx::NamedType) {
        let as_type = ASType::Alias(type_.name.as_str().to_string());
        let docs = &type_.docs;
        if docs.is_empty() {
            self.w.write_line(&format!("/** {} */", as_type));
        } else {
            write_docs(&mut self.w, &type_.docs);
        }
        let tref = &type_.tref;
        match tref {
            witx::TypeRef::Name(other_type) => {
                self.define_as_alias(&as_type, &other_type.as_ref().into())
            }
            witx::TypeRef::Value(witx_type) => {
                self.define_as_witx_type(&as_type, &witx_type.as_ref())
            }
        };
        self.w.eob();
    }

    fn define_module(&mut self, module: &witx::Module) {
        self.w.eob().write_line(&format!(
            "// ----------------------[{}]----------------------",
            module.name.as_str()
        ));
        for func in module.funcs() {
            self.define_func(module.name.as_str(), func.as_ref());
            self.w.eob();
        }
    }

    fn define_func(&mut self, module_name: &str, func: &witx::InterfaceFunc) {
        let docs = &func.docs;
        let name = func.name.as_str();
        if docs.is_empty() {
            self.w.write_line(&format!("\n/** {} */", name));
        } else {
            write_docs(&mut self.w, docs);
        }
        let s_in: Vec<_> = func
            .params
            .iter()
            .map(|param| param.name.as_str().to_string())
            .collect();
        let s_out: Vec<_> = func
            .results
            .iter()
            .map(|param| param.name.as_str().to_string())
            .collect();
        let module_name = match self.module_name.as_ref() {
            None => module_name,
            Some(module_name) => module_name.as_str(),
        };
        self.w.write_line("/**")
            .write_line(&format!(" * in:  {}", s_in.join(", ")))
            .write_line(&format!(" * out: {}", s_out.join(", ")))
            .write_line(" */");
        self.w.write_line("// @ts-ignore: decorator")
            .write_line(&format!("@external(\"{}\", \"{}\")", module_name, name))
            .write_line(&format!("export declare function {}(", name));

        let params = &func.params;
        let as_params = Self::params_to_as(params);
        let results = &func.results;
        let as_results = Self::params_to_as(results);
        let return_value = as_results.get(0);
        let as_results = if as_results.is_empty() {
            &[]
        } else {
            &as_results[1..]
        };
        let as_params: Vec<_> = as_params
            .iter()
            .map(|(v, t)| format!("{}: {}", v, t))
            .collect();
        let as_results: Vec<_> = as_results
            .iter()
            .map(|(v, t)| format!("{}_ptr: {}", v, ASType::MutPtr(Box::new(t.clone()))))
            .collect();
        if !as_params.is_empty() {
            if !as_results.is_empty() {
                self.w.continuation()
                    .write(&as_params.join(", "))
                    .write(",")
                    .eol();
            } else {
                self.w.continuation().write_line(&as_params.join(", "));
            }
        }
        println!("{:?}", return_value);
        let return_as_type_and_comment = match return_value {
            None => (ASType::Void, "".to_string()),
            Some(x) => (x.1.clone(), format!(" /* {} */", x.0)),
        };
        if !as_results.is_empty() {
            self.w.continuation().write_line(&as_results.join(", "));
        }
        self.w.write_line(&format!(
            "): {}{};",
            return_as_type_and_comment.0, return_as_type_and_comment.1
        ));
    }

    fn params_to_as(params: &[witx::InterfaceFuncParam]) -> Vec<(String, ASType)> {
        let mut as_params = vec![];
        for param in params {
            let leaf_type = Self::leaf_type(&param.tref);
            let as_leaf_type = ASType::from(leaf_type).name(param.tref.type_name());
            let (first, second) = as_leaf_type.decompose();
            match &param.tref {
                witx::TypeRef::Name(name) => {
                    as_params.push((
                        format!("{}{}", param.name.as_str(), first.1),
                        ASType::from(name.as_ref()),
                    ));
                }
                _ => {
                    as_params.push((format!("{}{}", param.name.as_str(), first.1), first.0));
                }
            }
            if let Some(second) = second {
                as_params.push((format!("{}{}", param.name.as_str(), second.1), second.0))
            }
        }
        as_params
    }

    fn leaf_type(type_ref: &witx::TypeRef) -> &witx::Type {
        match type_ref {
            witx::TypeRef::Name(other_type) => {
                let x = other_type.as_ref();
                Self::leaf_type(&x.tref)
            }
            witx::TypeRef::Value(type_) => type_.as_ref(),
        }
    }
}

fn define_variant_case(
    w: &mut PrettyWriter,
    as_type: &ASType,
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
    define_variant_case_accessors(w, as_type, i, variant);
}

fn define_variant_case_accessors(
    w: &mut PrettyWriter,
    as_type: &ASType,
    i: usize,
    variant: &witx::Case,
) {
    let variant_name = variant.name.as_str();
    match variant.tref.as_ref() {
        None => {
            w.write_line(&format!("static {}(): {} {{", variant_name, as_type))
                .indent()
                .write_line(&format!("return {}.new({});", as_type, i))
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
                variant_name, as_variant_type, as_type
            ));
            w.with_block(|w| {
                w.write_line(&format!("return {}.new({}, val);", as_type, i));
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