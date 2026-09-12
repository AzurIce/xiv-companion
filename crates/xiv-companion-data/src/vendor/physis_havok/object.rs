// SPDX-FileCopyrightText: 2020 Inseok Lee
// SPDX-License-Identifier: MIT
// derived from physis (https://github.com/AzurIce/physis), MIT license
// vendored into xiv-companion-data so the havok tagfile reader can be maintained locally.

#![allow(clippy::bad_bit_mask)]
#![allow(dead_code)]

use core::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

// NOTE(vendored): replaced the `bitflags` dependency with an equivalent hand-written
// impl so the vendored module stays dependency-free. Public surface kept identical
// (associated consts + `bits`/`from_bits`).
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct HavokValueType(u32);

impl HavokValueType {
    pub const EMPTY: Self = Self(0);
    pub const BYTE: Self = Self(1);
    pub const INT: Self = Self(2);
    pub const REAL: Self = Self(3);
    pub const VEC4: Self = Self(4);
    pub const VEC8: Self = Self(5);
    pub const VEC12: Self = Self(6);
    pub const VEC16: Self = Self(7);
    pub const OBJECT: Self = Self(8);
    pub const STRUCT: Self = Self(9);
    pub const STRING: Self = Self(10);

    pub const ARRAY: Self = Self(0x10);
    pub const ARRAYBYTE: Self = Self(Self::ARRAY.0 | Self::BYTE.0);
    pub const ARRAYINT: Self = Self(Self::ARRAY.0 | Self::INT.0);
    pub const ARRAYREAL: Self = Self(Self::ARRAY.0 | Self::REAL.0);
    pub const ARRAYVEC4: Self = Self(Self::ARRAY.0 | Self::VEC4.0);
    pub const ARRAYVEC8: Self = Self(Self::ARRAY.0 | Self::VEC8.0);
    pub const ARRAYVEC12: Self = Self(Self::ARRAY.0 | Self::VEC12.0);
    pub const ARRAYVEC16: Self = Self(Self::ARRAY.0 | Self::VEC16.0);
    pub const ARRAYOBJECT: Self = Self(Self::ARRAY.0 | Self::OBJECT.0);
    pub const ARRAYSTRUCT: Self = Self(Self::ARRAY.0 | Self::STRUCT.0);
    pub const ARRAYSTRING: Self = Self(Self::ARRAY.0 | Self::STRING.0);

    pub const TUPLE: Self = Self(0x20);
    pub const TUPLEBYTE: Self = Self(Self::TUPLE.0 | Self::BYTE.0);
    pub const TUPLEINT: Self = Self(Self::TUPLE.0 | Self::INT.0);
    pub const TUPLEREAL: Self = Self(Self::TUPLE.0 | Self::REAL.0);
    pub const TUPLEVEC4: Self = Self(Self::TUPLE.0 | Self::VEC4.0);
    pub const TUPLEVEC8: Self = Self(Self::TUPLE.0 | Self::VEC8.0);
    pub const TUPLEVEC12: Self = Self(Self::TUPLE.0 | Self::VEC12.0);
    pub const TUPLEVEC16: Self = Self(Self::TUPLE.0 | Self::VEC16.0);
    pub const TUPLEOBJECT: Self = Self(Self::TUPLE.0 | Self::OBJECT.0);
    pub const TUPLESTRUCT: Self = Self(Self::TUPLE.0 | Self::STRUCT.0);
    pub const TUPLESTRING: Self = Self(Self::TUPLE.0 | Self::STRING.0);

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub fn from_bits(bits: u32) -> Option<Self> {
        // matches bitflags' `from_bits`: reject bits outside the defined flag values
        // (union of all consts above = 0x3f).
        if bits & !0x3f == 0 {
            Some(Self(bits))
        } else {
            None
        }
    }
}

impl HavokValueType {
    pub fn is_tuple(self) -> bool {
        (self.bits() & HavokValueType::TUPLE.bits()) != 0
    }

    pub fn is_array(self) -> bool {
        (self.bits() & HavokValueType::ARRAY.bits()) != 0
    }

    pub fn base_type(self) -> HavokValueType {
        HavokValueType::from_bits(self.bits() & 0x0f).unwrap()
    }

    pub fn is_vec(self) -> bool {
        let base_type = self.base_type();
        base_type == HavokValueType::VEC4
            || base_type == HavokValueType::VEC8
            || base_type == HavokValueType::VEC12
            || base_type == HavokValueType::VEC16
    }

    pub fn vec_size(self) -> u8 {
        match self.base_type() {
            HavokValueType::VEC4 => 4,
            HavokValueType::VEC8 => 8,
            HavokValueType::VEC12 => 12,
            HavokValueType::VEC16 => 16,
            _ => panic!(),
        }
    }
}

pub type HavokInteger = i32;
pub type HavokReal = f32;

#[derive(Debug)]
pub enum HavokValue {
    Integer(HavokInteger),
    Real(HavokReal),
    String(Arc<str>),
    Vec(Vec<HavokReal>),
    Array(Vec<HavokValue>),
    Object(Arc<RefCell<HavokObject>>),

    ObjectReference(usize),
}

impl HavokValue {
    pub fn as_int(&self) -> HavokInteger {
        match self {
            Self::Integer(x) => *x,
            _ => panic!(),
        }
    }

    pub fn as_object(&self) -> Arc<RefCell<HavokObject>> {
        match self {
            Self::Object(x) => x.clone(),
            _ => panic!(),
        }
    }

    pub fn as_array(&self) -> &Vec<HavokValue> {
        match self {
            Self::Array(x) => x,
            _ => panic!(),
        }
    }

    pub fn as_string(&self) -> &str {
        match self {
            Self::String(x) => x,
            _ => panic!(),
        }
    }

    pub fn as_vec(&self) -> &Vec<HavokReal> {
        match self {
            Self::Vec(x) => x,
            _ => panic!(),
        }
    }

    pub fn as_real(&self) -> HavokReal {
        match self {
            Self::Real(x) => *x,
            _ => panic!(),
        }
    }
}

#[derive(Debug)]
pub struct HavokRootObject {
    object: Arc<RefCell<HavokObject>>,
}

impl HavokRootObject {
    pub fn new(object: Arc<RefCell<HavokObject>>) -> Self {
        Self { object }
    }

    pub fn find_object_by_type(&self, type_name: &'static str) -> Arc<RefCell<HavokObject>> {
        let root_obj = self.object.borrow();
        let named_variants = root_obj.get("namedVariants");

        for variant in named_variants.as_array() {
            let variant_obj = variant.as_object();
            if variant_obj.borrow().get("className").as_string() == type_name {
                return variant_obj.borrow().get("variant").as_object();
            }
        }
        unreachable!()
    }
}

#[derive(Debug)]
pub struct HavokObjectTypeMember {
    pub name: Arc<str>,
    pub type_: HavokValueType,
    pub tuple_size: u32,
    pub class_name: Option<Arc<str>>,
}

impl HavokObjectTypeMember {
    pub fn new(
        name: Arc<str>,
        type_: HavokValueType,
        tuple_size: u32,
        type_name: Option<Arc<str>>,
    ) -> Self {
        Self {
            name,
            type_,
            tuple_size,
            class_name: type_name,
        }
    }
}

#[derive(Debug)]
pub struct HavokObjectType {
    pub name: Arc<str>,
    parent: Option<Arc<HavokObjectType>>,
    members: Vec<HavokObjectTypeMember>,
}

impl HavokObjectType {
    pub fn new(
        name: Arc<str>,
        parent: Option<Arc<HavokObjectType>>,
        members: Vec<HavokObjectTypeMember>,
    ) -> Self {
        Self {
            name,
            parent,
            members,
        }
    }

    pub fn members(&self) -> Vec<&HavokObjectTypeMember> {
        if let Some(x) = &self.parent {
            x.members()
                .into_iter()
                .chain(self.members.iter())
                .collect::<Vec<_>>()
        } else {
            self.members.iter().collect::<Vec<_>>()
        }
    }

    pub fn member_count(&self) -> usize {
        (if let Some(x) = &self.parent {
            x.members.len()
        } else {
            0
        }) + self.members.len()
    }
}

#[derive(Debug)]
pub struct HavokObject {
    pub object_type: Arc<HavokObjectType>,
    data: HashMap<usize, HavokValue>,
}

impl HavokObject {
    pub fn new(object_type: Arc<HavokObjectType>, data: HashMap<usize, HavokValue>) -> Self {
        Self { object_type, data }
    }

    pub fn set(&mut self, index: usize, value: HavokValue) {
        self.data.insert(index, value);
    }

    pub fn get(&self, member_name: &str) -> &HavokValue {
        let member_index = self
            .object_type
            .members()
            .iter()
            .position(|&x| &*x.name == member_name)
            .unwrap();

        self.data.get(&member_index).unwrap()
    }

    pub(crate) fn members_mut(&mut self) -> impl Iterator<Item = (&usize, &mut HavokValue)> {
        self.data.iter_mut()
    }
}
