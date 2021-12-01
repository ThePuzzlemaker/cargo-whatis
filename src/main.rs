use std::collections::HashSet;

use cargo::{
    core::{Dependency, PackageId, PackageSet, Source, SourceId, SourceMap},
    sources::RegistrySource,
    Config,
};
use clap::{App, AppSettings, Arg, SubCommand};

fn main() -> anyhow::Result<()> {
    let matches = App::new("cargo-whatis")
        .version(env!("CARGO_PKG_VERSION"))
        .author("ThePuzzlemaker <tpzker@thepuzzlemaker.info>")
        .about("Quickly show the description of a crate on crates.io")
        .bin_name("cargo")
        .subcommand(
            SubCommand::with_name("whatis")
                .version(env!("CARGO_PKG_VERSION"))
                .author("ThePuzzlemaker <tpzker@thepuzzlemaker.info>")
                .about("Quickly show the description of a crate on crates.io")
                .arg(
                    Arg::with_name("crate")
                        .required(true)
                        .index(1)
                        .help("The crate to look up"),
                )
                .arg(
                    Arg::with_name("version")
                        .long("version")
                        .short("v")
                        .value_name("semver")
                        .help("Which version of the specified crate should be looked up")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("deps")
                        .long("deps")
                        .short("d")
                        .help("Show descriptions of (direct) dependencies of the provided crate"),
                ),
        )
        .setting(AppSettings::SubcommandRequired)
        .get_matches();

    let matches = matches.subcommand_matches("whatis").unwrap();

    let show_deps = matches.is_present("deps");

    let krate = matches.value_of("crate").unwrap();
    let version = matches.value_of("version");

    // let mut index = Index::new_cargo_default()?;
    // index.update()?;

    // let krate = index
    //     .crate_(krate)
    //     .ok_or_else(|| eyre::eyre!("Failed to find crate `{}`", krate))?;

    let cargo_cfg = Config::default()?;
    let _pkg_cache_lock = cargo_cfg.acquire_package_cache_lock()?;

    let crates_io = SourceId::crates_io(&cargo_cfg)?;

    let mut registry = RegistrySource::remote(crates_io, &HashSet::new(), &cargo_cfg);

    registry.update()?;

    let krate_dep = Dependency::parse(krate, version, crates_io)?;

    let krate_summary = registry
        .query_vec(&krate_dep)?
        .into_iter()
        .max_by_key(|s| s.version().clone())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Failed to find crate `{} = \"{}\"`",
                krate,
                krate_dep.version_req()
            )
        })?;

    let main_pkgid = PackageId::pure(
        krate_summary.name(),
        krate_summary.version().clone(),
        crates_io,
    );

    let mut pkgids = vec![main_pkgid];

    if show_deps {
        krate_summary
            .dependencies()
            .iter()
            .try_for_each(|dep| -> anyhow::Result<()> {
                let summary = registry
                    .query_vec(dep)?
                    .into_iter()
                    .max_by_key(|s| s.version().clone())
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Failed to find crate `{} = \"{}\"`",
                            krate,
                            dep.version_req()
                        )
                    })?;
                pkgids.push(PackageId::pure(
                    summary.name(),
                    summary.version().clone(),
                    crates_io,
                ));
                Ok(())
            })?;
    }

    let mut source_map = SourceMap::new();
    source_map.insert(Box::new(registry));

    let pkgset = PackageSet::new(&pkgids, source_map, &cargo_cfg)?;
    let mut dls = pkgset.enable_download()?;
    for pkg in pkgset.package_ids() {
        if dls.start(pkg)?.is_none() {
            dls.wait()?;
        }
    }

    let main_pkg = pkgset.get_one(main_pkgid)?;
    println!(
        "{} @ {}: {}",
        main_pkg.manifest().name(),
        main_pkg.version(),
        main_pkg
            .manifest()
            .metadata()
            .description
            .as_deref()
            .unwrap_or("No description provided.")
            .trim_end()
    );

    if show_deps {
        println!("\n # Dependencies:\n");
        for pkg in pkgset.packages() {
            if pkg.package_id() == main_pkgid {
                continue;
            }
            println!(
                "- {} @ {}: {}",
                pkg.manifest().name(),
                pkg.version(),
                pkg.manifest()
                    .metadata()
                    .description
                    .as_deref()
                    .unwrap_or("No description provided.")
                    .trim_end()
            )
        }
    }
    // TODO: Maybe find a way to only download the manifest, or delete crates downloaded afterwards to save space
    // TODO: Maybe add an option to parse a manifest and show the description of all its dependencies

    Ok(())
}
