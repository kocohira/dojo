use std::str::FromStr;

use anyhow::Result;
use clap::Args;
use dojo_utils::env::DOJO_WORLD_ADDRESS_ENV_VAR;
use dojo_world::config::Environment;
use starknet::core::types::Felt;
use tracing::trace;

#[derive(Debug, Args, Clone)]
#[command(next_help_heading = "World options")]
pub struct WorldOptions {
    #[arg(help = "The address of the World contract.")]
    #[arg(long = "world", env = DOJO_WORLD_ADDRESS_ENV_VAR)]
    #[arg(global = true)]
    pub world_address: Option<Felt>,

    #[arg(long, default_value = "false")]
    #[arg(help = "Whether the migration is a guest migration, which means the migration is \
                  performed on a world you are not the owner of.")]
    pub guest: bool,

    #[arg(
        long,
        help = "Whitelisted namespaces, separated by commas. If not provided, all namespaces will \
                be fetched."
    )]
    #[arg(value_delimiter = ',', num_args = 0..)]
    pub namespaces: Vec<String>,
}

impl WorldOptions {
    pub fn address(&self, env_metadata: Option<&Environment>) -> Result<Option<Felt>> {
        if let Some(world_address) = self.world_address {
            trace!(?world_address, "Loaded world_address.");
            Ok(Some(world_address))
        } else if let Some(world_address) = env_metadata.and_then(|env| env.world_address()) {
            trace!(world_address, "Loaded world_address from env metadata.");
            Ok(Some(Felt::from_str(world_address)?))
        } else {
            trace!("No world address found.");
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {

    use std::str::FromStr;

    use clap::Parser;
    use starknet_crypto::Felt;

    use super::{WorldOptions, DOJO_WORLD_ADDRESS_ENV_VAR};

    #[derive(clap::Parser, Debug)]
    struct Command {
        #[clap(flatten)]
        pub inner: WorldOptions,
    }
    #[test]

    fn world_address_read_from_env_variable() {
        std::env::set_var(DOJO_WORLD_ADDRESS_ENV_VAR, "0x0");

        let cmd = Command::parse_from([""]);
        assert_eq!(cmd.inner.world_address, Some(Felt::from_str("0x0").unwrap()));
    }

    #[test]
    fn world_address_from_args() {
        let cmd = Command::parse_from(["sozo", "--world", "0x0"]);
        assert_eq!(cmd.inner.address(None).unwrap(), Some(Felt::from_str("0x0").unwrap()));
    }

    #[test]
    fn world_address_from_env_metadata() {
        let env_metadata = dojo_world::config::Environment {
            world_address: Some("0x0".to_owned()),
            ..Default::default()
        };

        let cmd = Command::parse_from([""]);
        assert_eq!(
            cmd.inner.address(Some(&env_metadata)).unwrap(),
            Some(Felt::from_hex("0x0").unwrap())
        );
    }

    #[test]
    fn world_address_from_both() {
        let env_metadata = dojo_world::config::Environment {
            world_address: Some("0x0".to_owned()),
            ..Default::default()
        };

        let cmd = Command::parse_from(["sozo", "--world", "0x1"]);
        assert_eq!(
            cmd.inner.address(Some(&env_metadata)).unwrap(),
            Some(Felt::from_hex("0x1").unwrap())
        );
    }

    #[test]
    fn world_address_from_neither() {
        let cmd = Command::parse_from([""]);
        assert!(cmd.inner.address(None).is_ok());
    }
}
