//! `sde-convert` — turn CCP's Static Data Export YAML into the prebuilt
//! `sde.sqlite` that ships with EVE Commander.
//!
//! ```text
//! sde-convert --out sde.sqlite [--types typeIDs.yaml] [--systems systems.yaml]
//! ```
//!
//! At least one of `--types` / `--systems` must be given. The output file is
//! recreated from scratch each run so builds are deterministic.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use sde_tools::Converter;

struct Args {
    out: PathBuf,
    types: Option<PathBuf>,
    systems: Option<PathBuf>,
    type_materials: Option<PathBuf>,
    blueprints: Option<PathBuf>,
    skills: Option<PathBuf>,
    required_skills: Option<PathBuf>,
}

fn parse_args() -> Result<Args> {
    let mut out = None;
    let mut types = None;
    let mut systems = None;
    let mut type_materials = None;
    let mut blueprints = None;
    let mut skills = None;
    let mut required_skills = None;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--out" | "-o" => out = Some(PathBuf::from(next_value(&mut it, &arg)?)),
            "--types" => types = Some(PathBuf::from(next_value(&mut it, &arg)?)),
            "--systems" => systems = Some(PathBuf::from(next_value(&mut it, &arg)?)),
            "--type-materials" => {
                type_materials = Some(PathBuf::from(next_value(&mut it, &arg)?))
            }
            "--blueprints" => blueprints = Some(PathBuf::from(next_value(&mut it, &arg)?)),
            "--skills" => skills = Some(PathBuf::from(next_value(&mut it, &arg)?)),
            "--required-skills" => {
                required_skills = Some(PathBuf::from(next_value(&mut it, &arg)?))
            }
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            other => bail!("unknown argument: {other} (try --help)"),
        }
    }

    let out = out.context("missing required --out <path>")?;
    if types.is_none()
        && systems.is_none()
        && type_materials.is_none()
        && blueprints.is_none()
        && skills.is_none()
        && required_skills.is_none()
    {
        bail!("nothing to do: pass at least one input (--types/--systems/--type-materials/--blueprints/--skills/--required-skills)");
    }
    Ok(Args { out, types, systems, type_materials, blueprints, skills, required_skills })
}

fn next_value(it: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    it.next().with_context(|| format!("{flag} requires a value"))
}

fn print_usage() {
    eprintln!(
        "sde-convert — CCP SDE YAML → sde.sqlite\n\n\
         USAGE:\n  \
         sde-convert --out <sde.sqlite> [--types <typeIDs.yaml>] [--systems <systems.yaml>]\n  \
         [--type-materials <typeMaterials.yaml>] [--blueprints <blueprints.yaml>] [--skills <skills.yaml>] [--required-skills <reqSkills.yaml>]\n\n\
         At least one input is required."
    );
}

fn run() -> Result<()> {
    let args = parse_args()?;
    let mut conv = Converter::create(&args.out)
        .with_context(|| format!("creating {}", args.out.display()))?;

    if let Some(p) = &args.types {
        let yaml = std::fs::read_to_string(p)
            .with_context(|| format!("reading {}", p.display()))?;
        let n = conv.ingest_types(&yaml)?;
        eprintln!("types:   {n} rows from {}", p.display());
    }
    if let Some(p) = &args.systems {
        let yaml = std::fs::read_to_string(p)
            .with_context(|| format!("reading {}", p.display()))?;
        let n = conv.ingest_systems(&yaml)?;
        eprintln!("systems: {n} rows from {}", p.display());
    }
    if let Some(p) = &args.type_materials {
        let yaml = std::fs::read_to_string(p)
            .with_context(|| format!("reading {}", p.display()))?;
        let n = conv.ingest_type_materials(&yaml)?;
        eprintln!("type_materials: {n} rows from {}", p.display());
    }
    if let Some(p) = &args.blueprints {
        let yaml = std::fs::read_to_string(p)
            .with_context(|| format!("reading {}", p.display()))?;
        let n = conv.ingest_blueprints(&yaml)?;
        eprintln!("blueprints: {n} rows from {}", p.display());
    }
    if let Some(p) = &args.skills {
        let yaml = std::fs::read_to_string(p)
            .with_context(|| format!("reading {}", p.display()))?;
        let n = conv.ingest_skills(&yaml)?;
        eprintln!("skills: {n} rows from {}", p.display());
    }
    if let Some(p) = &args.required_skills {
        let yaml = std::fs::read_to_string(p)
            .with_context(|| format!("reading {}", p.display()))?;
        let n = conv.ingest_required_skills(&yaml)?;
        eprintln!("required_skills: {n} rows from {}", p.display());
    }

    eprintln!("wrote {}", args.out.display());
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}
