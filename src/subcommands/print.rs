use anyhow::Result;
use clap::Parser;

use crate::{spec::Specification, GenerationProfile, SpecVersion};

#[derive(Debug, Parser)]
pub struct Print {
    #[clap(long, env, help = "Version of the specification")]
    spec: SpecVersion,
    #[clap(long, help = "Sort component definitions")]
    sort: bool,
}

impl Print {
    pub(crate) fn run(self, profiles: &[GenerationProfile]) -> Result<()> {
        let profile = profiles
            .iter()
            .find(|profile| profile.version == self.spec)
            .expect("Unable to find profile");

        let mut main_specs: Specification =
            serde_json::from_str(profile.raw_specs.main).expect("Failed to parse specification");

        if self.sort {
            main_specs.components.schemas.sort_keys();
            main_specs.components.errors.sort_keys();
        }

        println!(
            "{}",
            serde_json::to_string_pretty(&main_specs).expect("Failed to serialize specification")
        );

        Ok(())
    }
}
