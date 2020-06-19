// Copyright 2018-2020 Parity Technologies (UK) Ltd.
// This file is part of cargo-contract.
//
// ink! is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// ink! is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with ink!.  If not, see <http://www.gnu.org/licenses/>.

use std::{fs, io::Read, path::PathBuf};

use anyhow::{Context, Result};
use sp_core::H256;
use subxt::{contracts::*, ClientBuilder, DefaultNodeRuntime};

use crate::{cmd::build, ExtrinsicOpts};

/// Load the wasm blob from the specified path.
///
/// Defaults to the target contract wasm in the current project, inferred via the crate metadata.
fn load_contract_code(path: Option<&PathBuf>) -> Result<Vec<u8>> {
    let contract_wasm_path = match path {
        Some(path) => path.clone(),
        None => build::collect_crate_metadata(&Default::default())?.dest_wasm,
    };
    log::info!("Contract code path: {}", contract_wasm_path.display());
    let mut data = Vec::new();
    let mut file = fs::File::open(&contract_wasm_path)
        .context(format!("Failed to open {}", contract_wasm_path.display()))?;
    file.read_to_end(&mut data)?;

    Ok(data)
}

/// Put contract code to a smart contract enabled substrate chain.
/// Returns the code hash of the deployed contract if successful.
///
/// Optionally supply the contract wasm path, defaults to destination contract file inferred from
/// Cargo.toml of the current contract project.
///
/// Creates an extrinsic with the `Contracts::put_code` Call, submits via RPC, then waits for
/// the `ContractsEvent::CodeStored` event.
pub(crate) fn execute_deploy(
    extrinsic_opts: &ExtrinsicOpts,
    contract_wasm_path: Option<&PathBuf>,
) -> Result<H256> {
    let code = load_contract_code(contract_wasm_path)?;

    async_std::task::block_on(async move {
        let cli = ClientBuilder::<DefaultNodeRuntime>::new()
            .set_url(&extrinsic_opts.url.to_string())
            .build()
            .await?;
        let signer = extrinsic_opts.signer()?;
        let xt = cli.xt(signer, None).await?;

        let events = xt.watch().put_code(&code).await?;
        let code_stored = events
            .code_stored()?
            .ok_or(anyhow::anyhow!("Failed to find CodeStored event"))?;

        Ok(code_stored.code_hash)
    })
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write};

    use crate::{cmd::deploy::execute_deploy, util::tests::with_tmp_dir, ExtrinsicOpts};
    use assert_matches::assert_matches;

    const CONTRACT: &str = r#"
(module
    (func (export "call"))
    (func (export "deploy"))
)
"#;

    #[test]
    #[ignore] // depends on a local substrate node running
    fn deploy_contract() {
        with_tmp_dir(|path| {
            let wasm = wabt::wat2wasm(CONTRACT).expect("invalid wabt");

            let wasm_path = path.join("test.wasm");
            let mut file = fs::File::create(&wasm_path).unwrap();
            let _ = file.write_all(&wasm);

            let url = url::Url::parse("ws://localhost:9944").unwrap();
            let extrinsic_opts = ExtrinsicOpts {
                url,
                suri: "//Alice".into(),
                password: None,
            };
            let result = execute_deploy(&extrinsic_opts, Some(&wasm_path));

            assert_matches!(result, Ok(_));
        });
    }
}
