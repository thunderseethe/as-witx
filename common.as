
export type handle = i32;
export type char = u8;
export type ptr<T> = usize;
export type mut_ptr<T> = usize;
export type untyped_ptr = usize;
export type struct<T> = usize;
export type union<T> = usize;
export type wasi_string_ptr = ptr<char>;

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