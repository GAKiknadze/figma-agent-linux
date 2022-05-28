#![allow(non_upper_case_globals, clippy::missing_safety_doc)]

use std::{
    ffi::{CStr, CString},
    marker::PhantomData,
    mem::size_of,
    ptr, slice, str,
};

use libc::{c_long, c_void, free, malloc, realloc};

pub use freetype_sys::*;

#[macro_export]
macro_rules! dispatch {
    ($expr:expr) => {{
        let result = $expr;
        assert!(result == FT_Err_Ok);
    }};
}

#[macro_export]
macro_rules! try_dispatch {
    ($expr:expr) => {{
        let result = $expr;
        if result == FT_Err_Ok {
            Ok(())
        } else {
            Err(result)
        }
    }};
}

pub struct Library {
    raw: FT_Library,
}

impl Library {
    pub fn new() -> Result<Library, FT_Error> {
        let mut raw: FT_Library = ptr::null_mut();
        try_dispatch!(unsafe { FT_New_Library(&mut MEMORY, &mut raw) })?;
        Ok(Library { raw })
    }

    pub unsafe fn from_raw(raw: FT_Library) -> Library {
        Library { raw }
    }

    pub unsafe fn from_raw_with_ref(raw: FT_Library) -> Library {
        dispatch!(FT_Reference_Library(raw));
        Library { raw }
    }

    pub fn init() -> Result<Library, FT_Error> {
        let library = Library::new()?;
        try_dispatch!(unsafe { FT_Add_Default_Modules(library.raw) })?;
        Ok(library)
    }

    pub fn new_face(&self, path: &str, face_index: i64) -> Result<Face, FT_Error> {
        Face::new(self, path, face_index)
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        dispatch!(unsafe { FT_Done_Library(self.raw) });
    }
}

pub struct Face<'a> {
    library: &'a Library,
    raw: FT_Face,
}

impl<'a> Face<'a> {
    pub fn new(library: &'a Library, path: &str, face_index: i64) -> Result<Face<'a>, FT_Error> {
        let mut raw: FT_Face = ptr::null_mut();
        let path = CString::new(path).unwrap();
        try_dispatch!(unsafe { FT_New_Face(library.raw, path.as_ptr(), face_index, &mut raw) })?;
        Ok(Face { library, raw })
    }

    pub unsafe fn from_raw(library: &'a Library, raw: FT_Face) -> Face<'a> {
        Face { library, raw }
    }

    pub unsafe fn from_raw_with_ref(library: &'a Library, raw: FT_Face) -> Face<'a> {
        dispatch!(FT_Reference_Face(raw));
        Face { library, raw }
    }

    pub fn sfnt_name_count(&self) -> usize {
        unsafe { FT_Get_Sfnt_Name_Count(self.raw) as usize }
    }

    pub fn sfnt_name_at(&self, index: usize) -> Result<SfntName, FT_Error> {
        let sfnt_name = SfntName::new();
        try_dispatch!(unsafe { FT_Get_Sfnt_Name(self.raw, index as FT_UInt, sfnt_name.raw) })?;
        Ok(sfnt_name)
    }

    pub fn mm_var(&self) -> Result<MMVar, FT_Error> {
        let mut raw_mm_var: *mut FT_MM_Var = ptr::null_mut();
        try_dispatch!(unsafe { FT_Get_MM_Var(self.raw, &mut raw_mm_var) })?;
        Ok(MMVar {
            raw: raw_mm_var,
            library: self.library,
            face: self,
        })
    }
}

impl Drop for Face<'_> {
    fn drop(&mut self) {
        dispatch!(unsafe { FT_Done_Face(self.raw) });
    }
}

pub struct SfntName<'a> {
    raw: *mut FT_SfntName,
    _marker: PhantomData<&'a Face<'a>>,
}

impl<'a> SfntName<'a> {
    pub fn new() -> SfntName<'a> {
        let raw = unsafe { malloc(size_of::<FT_SfntName>()) as *mut FT_SfntName };
        SfntName {
            raw,
            _marker: PhantomData,
        }
    }

    pub fn name(&self) -> &'a str {
        let slice =
            unsafe { slice::from_raw_parts((*self.raw).string, (*self.raw).string_len as usize) };
        str::from_utf8(slice).unwrap()
    }
}

impl Drop for SfntName<'_> {
    fn drop(&mut self) {
        unsafe { free(self.raw as *mut c_void) };
    }
}

pub struct MMVar<'a> {
    raw: *mut FT_MM_Var,
    library: &'a Library,
    face: &'a Face<'a>,
}

impl<'a> MMVar<'a> {
    pub fn num_axis(&self) -> usize {
        unsafe { (*self.raw).num_axis as usize }
    }

    pub fn num_designs(&self) -> usize {
        unsafe { (*self.raw).num_designs as usize }
    }

    pub fn num_named_styles(&self) -> usize {
        unsafe { (*self.raw).num_namedstyles as usize }
    }

    pub fn axis(&self) -> impl Iterator<Item = VarAxis> {
        let raw_axis =
            unsafe { slice::from_raw_parts((*self.raw).axis, (*self.raw).num_axis as usize) };
        raw_axis.iter().map(|axis| VarAxis {
            raw: axis,
            face: self.face,
        })
    }

    pub fn named_styles(&self) -> impl Iterator<Item = VarNamedStyle> {
        let raw_named_styles = unsafe {
            slice::from_raw_parts((*self.raw).namedstyle, (*self.raw).num_namedstyles as usize)
        };
        raw_named_styles.iter().map(|named_style| VarNamedStyle {
            raw: named_style,
            mm_var: self,
        })
    }
}

impl Drop for MMVar<'_> {
    fn drop(&mut self) {
        dispatch!(unsafe { FT_Done_MM_Var(self.library.raw, self.raw) });
    }
}

pub struct VarAxis<'a> {
    raw: &'a FT_Var_Axis,
    face: &'a Face<'a>,
}

impl<'a> VarAxis<'a> {
    pub fn name(&self) -> &str {
        unsafe { CStr::from_ptr(self.raw.name).to_str().unwrap() }
    }

    pub fn sfnt_name(&self) -> Option<&str> {
        Some(self.face.sfnt_name_at(self.raw.strid as usize).ok()?.name())
    }

    pub fn default(&self) -> i64 {
        self.raw.def
    }

    pub fn minimum(&self) -> i64 {
        self.raw.minimum
    }

    pub fn maximum(&self) -> i64 {
        self.raw.maximum
    }
}

pub struct VarNamedStyle<'a> {
    raw: &'a FT_Var_Named_Style,
    mm_var: &'a MMVar<'a>,
}

impl<'a> VarNamedStyle<'a> {
    pub fn coords(&self) -> impl Iterator<Item = i64> {
        let raw_coords = unsafe { slice::from_raw_parts(self.raw.coords, self.mm_var.num_axis()) };
        raw_coords.iter().map(|&coord| coord)
    }
}

static mut MEMORY: FT_MemoryRec = FT_MemoryRec {
    user: ptr::null_mut(),
    alloc: memory_alloc,
    realloc: memory_realloc,
    free: memory_free,
};

extern "C" fn memory_alloc(_: FT_Memory, size: c_long) -> *mut c_void {
    unsafe { malloc(size as usize) }
}

extern "C" fn memory_realloc(
    _: FT_Memory,
    _: c_long,
    size: c_long,
    pointer: *mut c_void,
) -> *mut c_void {
    unsafe { realloc(pointer, size as usize) }
}

extern "C" fn memory_free(_: FT_Memory, pointer: *mut c_void) {
    unsafe { free(pointer) }
}
