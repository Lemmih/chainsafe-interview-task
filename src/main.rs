use anyhow::Result;
use ethers::{prelude::*, utils::Ganache};
use ethers_solc::{Project, ProjectPathsConfig};
use std::{convert::TryFrom, path::PathBuf, sync::Arc, time::Duration};

use ipfs_api_backend_hyper::{IpfsApi, IpfsClient};
use std::fs::File;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let mut args = std::env::args();
    let prog_name = args.next().expect("failed to find program name.");

    match &args.collect::<Vec<String>>()[..] {
        [path] => {
            let cid = upload_to_ipfs(std::path::Path::new(path))
                .await
                .expect("failed to upload");
            store_cid_in_contract(cid)
                .await
                .expect("Failed to store CID in smart contract");
        }
        _ => {
            eprintln!("usage: {} file_path", prog_name);
            std::process::exit(-1);
        }
    }
}

async fn upload_to_ipfs(path: &std::path::Path) -> Result<String> {
    let client = IpfsClient::default();
    let file = File::open(path)?;

    Ok(client.add(file).await?.hash)
}

// Generate the type-safe contract bindings by providing the ABI
// definition in human readable format
abigen!(
    SimpleContract,
    r#"[
        function setCID(string)
        function getCID() external view returns (string)
    ]"#,
);

async fn store_cid_in_contract(cid: String) -> Result<()> {
    // the directory we use is root-dir
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // we use `root` for both the project root and for where to search for contracts since
    // everything is in the same directory
    let paths = ProjectPathsConfig::builder()
        .root(&root)
        .sources(&root)
        .build()?;
    // get the solc project instance using the paths above
    let solc = Project::builder()
        .paths(paths)
        .ephemeral()
        .no_artifacts()
        .build()?;
    // compile the project and get the artifacts
    let compiled = solc.compile()?.output();
    let path = root.join("contract.sol");
    let path = path.to_string_lossy();
    let contract = compiled
        .get(&path, "SimpleStorage")
        .expect("could not find contract");

    // 2. instantiate our wallet & ganache
    let ganache = Ganache::new().spawn();
    let wallet: LocalWallet = ganache.keys()[0].clone().into();

    // 3. connect to the network
    let provider =
        Provider::<Http>::try_from(ganache.endpoint())?.interval(Duration::from_millis(10u64));

    // 4. instantiate the client with the wallet
    let client = SignerMiddleware::new(provider, wallet);
    let client = Arc::new(client);

    // 5. create a factory which will be used to deploy instances of the contract
    let factory = ContractFactory::new(
        contract.abi.unwrap().clone(),
        contract.bin.unwrap().clone(),
        client.clone(),
    );

    // 6. deploy it with the constructor arguments
    let contract = factory
        .deploy("initial value".to_string())?
        .legacy()
        .send()
        .await?;

    // 7. get the contract's address
    let addr = contract.address();

    // 8. instantiate the contract
    let contract = SimpleContract::new(addr, client.clone());

    // 9. call the `setValue` method
    // (first `await` returns a PendingTransaction, second one waits for it to be mined)
    let _receipt = contract.set_cid(cid).legacy().send().await?.await?;

    // 10. get the new value
    let value = contract.get_cid().call().await?;

    println!("CID in smart contract: {}.", value);

    Ok(())
}
