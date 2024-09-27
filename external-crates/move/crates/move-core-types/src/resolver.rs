// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{account_address::AccountAddress, language_storage::ModuleId};
use std::{fmt::Debug, sync::Arc};

/// Traits for resolving Move modules and resources from persistent storage

/// A persistent storage backend that can resolve modules by address + name.
/// Storage backends should return
///   - Ok(Some(..)) if the data exists
///   - Ok(None)     if the data does not exist
///   - Err(..)      only when something really wrong happens, for example
///                    - invariants are broken and observable from the storage side
///                      (this is not currently possible as ModuleId and StructTag
///                       are always structurally valid)
///                    - storage encounters internal error
pub trait ModuleResolver {
    type Error: Debug;

    fn get_module(&self, id: &ModuleId) -> Result<Option<Vec<u8>>, Self::Error>;

    fn get_package(&self, id: &AccountAddress) -> Result<Option<Vec<Vec<u8>>>, Self::Error>;
}

/// A persistent storage implementation that can resolve both resources and modules
/// TODO(vm-rewrite): Remove this in favor of using the `ModuleResolver` trait directly.
pub trait MoveResolver: ModuleResolver<Error = Self::Err> {
    type Err: Debug;
}

impl<E: Debug, T: ModuleResolver<Error = E> + ?Sized> MoveResolver for T {
    type Err = E;
}

impl<T: ModuleResolver + ?Sized> ModuleResolver for &T {
    type Error = T::Error;
    fn get_module(&self, module_id: &ModuleId) -> Result<Option<Vec<u8>>, Self::Error> {
        (**self).get_module(module_id)
    }

    fn get_package(&self, id: &AccountAddress) -> Result<Option<Vec<Vec<u8>>>, Self::Error> {
        (**self).get_package(id)
    }
}

impl<T: ModuleResolver + ?Sized> ModuleResolver for Arc<T> {
    type Error = T::Error;
    fn get_module(&self, module_id: &ModuleId) -> Result<Option<Vec<u8>>, Self::Error> {
        (**self).get_module(module_id)
    }

    fn get_package(&self, id: &AccountAddress) -> Result<Option<Vec<Vec<u8>>>, Self::Error> {
        (**self).get_package(id)
    }
}
