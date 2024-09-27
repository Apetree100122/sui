// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    sandbox::utils::{
        explain_publish_changeset, get_gas_status, on_disk_state_view::OnDiskStateView,
    },
    NativeFunctionRecord,
};
use anyhow::{bail, Result};
use move_package::compilation::compiled_package::CompiledPackage;
use move_vm_runtime::{
    cache::linkage_context::LinkageContext, natives::functions::NativeFunctions,
    on_chain::ast::PackageStorageId, test_utils::gas_schedule::CostTable, vm::vm::VirtualMachine,
};
use std::collections::{BTreeSet, HashMap};

pub fn publish(
    natives: impl IntoIterator<Item = NativeFunctionRecord>,
    cost_table: &CostTable,
    state: &OnDiskStateView,
    package: &CompiledPackage,
    package_storage_id: &Option<PackageStorageId>,
    verbose: bool,
) -> Result<()> {
    // collect all modules compiled for the root package
    let compiled_modules = package.root_modules().collect::<Vec<_>>();
    if verbose {
        println!("Found {} modules", compiled_modules.len());
    }

    let root_package_addrs = compiled_modules
        .iter()
        .map(|module| *module.unit.module.self_id().address())
        .collect::<BTreeSet<_>>();
    if root_package_addrs.is_empty() {
        bail!("No modules to publish -- a package cannot be empty");
    }
    if root_package_addrs.len() != 1 {
        bail!("All modules in a package must have the same address");
    }

    let package_runtime_id = *root_package_addrs.iter().next().unwrap();
    let package_storage_id = package_storage_id.unwrap_or_else(|| package_runtime_id);

    // We don't allow republishing of packages
    if state.has_package(&package_storage_id) {
        bail!("Tried to republish the package at  {}. You will need to provide a different 'publish-at' address for the package", package_storage_id);
    }

    let compiled_modules = compiled_modules
        .into_iter()
        .map(|module| &module.unit)
        .collect::<Vec<_>>();

    // Build the dependency map from the package
    let mut dependency_map = HashMap::new();
    for (name, unit) in package.deps_compiled_units.iter() {
        let unit_address = *unit.unit.module.self_id().address();
        if let Some(other) = dependency_map.insert(unit_address, unit_address) {
            if other != unit_address {
                bail!(
                    "Package {name} has linkages: {} and {}",
                    other,
                    unit_address
                );
            }
        }
    }
    dependency_map.insert(package_runtime_id, package_storage_id);

    // use the publish_module API from the VM since we don't allow breaking changes
    let natives = NativeFunctions::new(natives)?;
    let mut vm = VirtualMachine::new_with_default_config(natives);

    let mut gas_status = get_gas_status(cost_table, None)?;

    // Create a `LinkageContext`
    let linkage_context = LinkageContext::new(package_storage_id, dependency_map);
    println!("LINKAGE CONTEXT: {:#?}", linkage_context);

    // Serialize the modules to prepare them for publishing
    let serialized_modules: Vec<Vec<u8>> = compiled_modules
        .iter()
        .map(|module| module.serialize())
        .collect();

    // Publish the package using the VM
    let (publish_result, _) = vm.publish_package(
        state,
        &linkage_context,
        package_runtime_id,
        package_storage_id,
        serialized_modules,
        &mut gas_status,
    );
    let changeset = publish_result?;
    if verbose {
        explain_publish_changeset(&changeset);
    }
    let modules: Vec<_> = changeset
        .into_modules()
        .map(|(module_id, blob_opt)| {
            (
                module_id.name().to_owned(),
                blob_opt.ok().expect("must be non-deletion"),
            )
        })
        .collect();
    state.save_package(&package_storage_id, modules)?;

    Ok(())
}
