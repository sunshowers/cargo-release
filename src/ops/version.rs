use std::str::FromStr;

use crate::error::FatalError;

#[derive(Clone, Debug)]
pub enum TargetVersion {
    Relative(BumpLevel),
    Absolute(semver::Version),
}

impl TargetVersion {
    pub fn bump(
        &self,
        current: &semver::Version,
        metadata: Option<&str>,
    ) -> Result<Option<Version>, FatalError> {
        let bumped = match self {
            TargetVersion::Relative(bump_level) => {
                let mut potential_version = current.to_owned();
                if bump_level.bump_version(&mut potential_version, metadata)? {
                    let full_version = potential_version;
                    let version = Version::from(full_version);
                    Some(version)
                } else {
                    None
                }
            }
            TargetVersion::Absolute(version) => {
                let mut full_version = version.to_owned();
                if full_version.build.is_empty() {
                    if let Some(metadata) = metadata {
                        full_version.build = semver::BuildMetadata::new(metadata)?;
                    } else {
                        full_version.build = current.build.clone();
                    }
                }
                let version = Version::from(full_version);
                if version.bare_version != Version::from(current.clone()).bare_version {
                    Some(version)
                } else {
                    None
                }
            }
        };
        Ok(bumped)
    }
}

impl Default for TargetVersion {
    fn default() -> Self {
        TargetVersion::Relative(BumpLevel::Release)
    }
}

impl std::fmt::Display for TargetVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            TargetVersion::Relative(bump_level) => {
                write!(f, "{}", bump_level)
            }
            TargetVersion::Absolute(version) => {
                write!(f, "{}", version)
            }
        }
    }
}

impl std::str::FromStr for TargetVersion {
    type Err = FatalError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(bump_level) = BumpLevel::from_str(s) {
            Ok(TargetVersion::Relative(bump_level))
        } else {
            Ok(TargetVersion::Absolute(semver::Version::parse(s)?))
        }
    }
}

impl clap::builder::ValueParserFactory for TargetVersion {
    type Parser = TargetVersionParser;

    fn value_parser() -> Self::Parser {
        TargetVersionParser
    }
}

#[derive(Copy, Clone)]
pub struct TargetVersionParser;

impl clap::builder::TypedValueParser for TargetVersionParser {
    type Value = TargetVersion;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let inner_parser = TargetVersion::from_str;
        inner_parser.parse_ref(cmd, arg, value)
    }

    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = clap::builder::PossibleValue> + '_>> {
        let inner_parser = clap::builder::EnumValueParser::<BumpLevel>::new();
        #[allow(clippy::needless_collect)] // Erasing a lifetime
        inner_parser.possible_values().map(|ps| {
            let ps = ps.collect::<Vec<_>>();
            let ps: Box<dyn Iterator<Item = clap::builder::PossibleValue> + '_> =
                Box::new(ps.into_iter());
            ps
        })
    }
}

#[derive(Debug, Clone)]
pub struct Version {
    pub full_version: semver::Version,
    pub full_version_string: String,
    pub bare_version: semver::Version,
    pub bare_version_string: String,
}

impl Version {
    pub fn is_prerelease(&self) -> bool {
        self.full_version.is_prerelease()
    }
}

impl From<semver::Version> for Version {
    fn from(full_version: semver::Version) -> Self {
        let full_version_string = full_version.to_string();
        let mut bare_version = full_version.clone();
        bare_version.build = semver::BuildMetadata::EMPTY;
        let bare_version_string = bare_version.to_string();
        Self {
            full_version,
            full_version_string,
            bare_version,
            bare_version_string,
        }
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum BumpLevel {
    /// Increase the major version (x.0.0)
    Major,
    /// Increase the minor version (x.y.0)
    Minor,
    /// Increase the patch version (x.y.z)
    Patch,
    /// Increase the rc pre-version (x.y.z-rc.M)
    Rc,
    /// Increase the beta pre-version (x.y.z-beta.M)
    Beta,
    /// Increase the alpha pre-version (x.y.z-alpha.M)
    Alpha,
    /// Remove the pre-version (x.y.z)
    Release,
}

impl std::fmt::Display for BumpLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use clap::ValueEnum;

        self.to_possible_value()
            .expect("no values are skipped")
            .get_name()
            .fmt(f)
    }
}

impl std::str::FromStr for BumpLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use clap::ValueEnum;

        for variant in Self::value_variants() {
            if variant.to_possible_value().unwrap().matches(s, false) {
                return Ok(*variant);
            }
        }
        Err(format!("Invalid variant: {}", s))
    }
}

impl BumpLevel {
    pub fn bump_version(
        self,
        version: &mut semver::Version,
        metadata: Option<&str>,
    ) -> Result<bool, FatalError> {
        let mut need_commit = false;
        match self {
            BumpLevel::Major => {
                version.increment_major();
                need_commit = true;
            }
            BumpLevel::Minor => {
                version.increment_minor();
                need_commit = true;
            }
            BumpLevel::Patch => {
                if !version.is_prerelease() {
                    version.increment_patch();
                } else {
                    version.pre = semver::Prerelease::EMPTY;
                }
                need_commit = true;
            }
            BumpLevel::Rc => {
                version.increment_rc()?;
                need_commit = true;
            }
            BumpLevel::Beta => {
                version.increment_beta()?;
                need_commit = true;
            }
            BumpLevel::Alpha => {
                version.increment_alpha()?;
                need_commit = true;
            }
            BumpLevel::Release => {
                if version.is_prerelease() {
                    version.pre = semver::Prerelease::EMPTY;
                    need_commit = true;
                }
            }
        };

        if let Some(metadata) = metadata {
            version.metadata(metadata)?;
        }

        Ok(need_commit)
    }
}

pub trait VersionExt {
    fn increment_major(&mut self);
    fn increment_minor(&mut self);
    fn increment_patch(&mut self);
    fn increment_alpha(&mut self) -> Result<(), FatalError>;
    fn increment_beta(&mut self) -> Result<(), FatalError>;
    fn increment_rc(&mut self) -> Result<(), FatalError>;
    fn prerelease_id_version(&self) -> Result<Option<(String, Option<u64>)>, FatalError>;
    fn metadata(&mut self, metadata: &str) -> Result<(), FatalError>;
    fn is_prerelease(&self) -> bool;
}

impl VersionExt for semver::Version {
    fn increment_major(&mut self) {
        self.major += 1;
        self.minor = 0;
        self.patch = 0;
        self.pre = semver::Prerelease::EMPTY;
        self.build = semver::BuildMetadata::EMPTY;
    }

    fn increment_minor(&mut self) {
        self.minor += 1;
        self.patch = 0;
        self.pre = semver::Prerelease::EMPTY;
        self.build = semver::BuildMetadata::EMPTY;
    }

    fn increment_patch(&mut self) {
        self.patch += 1;
        self.pre = semver::Prerelease::EMPTY;
        self.build = semver::BuildMetadata::EMPTY;
    }

    fn prerelease_id_version(&self) -> Result<Option<(String, Option<u64>)>, FatalError> {
        if !self.pre.is_empty() {
            if let Some((alpha, numeric)) = self.pre.as_str().split_once('.') {
                let alpha = alpha.to_owned();
                let numeric = u64::from_str(numeric)
                    .map_err(|_| FatalError::UnsupportedPrereleaseVersionScheme)?;
                Ok(Some((alpha, Some(numeric))))
            } else {
                Ok(Some((self.pre.as_str().to_owned(), None)))
            }
        } else {
            Ok(None)
        }
    }

    fn increment_alpha(&mut self) -> Result<(), FatalError> {
        if let Some((pre_ext, pre_ext_ver)) = self.prerelease_id_version()? {
            if pre_ext == VERSION_BETA || pre_ext == VERSION_RC {
                Err(FatalError::InvalidReleaseLevel(VERSION_ALPHA.to_owned()))
            } else {
                let new_ext_ver = if pre_ext == VERSION_ALPHA {
                    pre_ext_ver.unwrap_or(0) + 1
                } else {
                    1
                };
                self.pre = semver::Prerelease::new(&format!("{}.{}", VERSION_ALPHA, new_ext_ver))?;
                Ok(())
            }
        } else {
            self.increment_patch();
            self.pre = semver::Prerelease::new(&format!("{}.1", VERSION_ALPHA))?;
            Ok(())
        }
    }

    fn increment_beta(&mut self) -> Result<(), FatalError> {
        if let Some((pre_ext, pre_ext_ver)) = self.prerelease_id_version()? {
            if pre_ext == VERSION_RC {
                Err(FatalError::InvalidReleaseLevel(VERSION_BETA.to_owned()))
            } else {
                let new_ext_ver = if pre_ext == VERSION_BETA {
                    pre_ext_ver.unwrap_or(0) + 1
                } else {
                    1
                };
                self.pre = semver::Prerelease::new(&format!("{}.{}", VERSION_BETA, new_ext_ver))?;
                Ok(())
            }
        } else {
            self.increment_patch();
            self.pre = semver::Prerelease::new(&format!("{}.1", VERSION_BETA))?;
            Ok(())
        }
    }

    fn increment_rc(&mut self) -> Result<(), FatalError> {
        if let Some((pre_ext, pre_ext_ver)) = self.prerelease_id_version()? {
            let new_ext_ver = if pre_ext == VERSION_RC {
                pre_ext_ver.unwrap_or(0) + 1
            } else {
                1
            };
            self.pre = semver::Prerelease::new(&format!("{}.{}", VERSION_RC, new_ext_ver))?;
            Ok(())
        } else {
            self.increment_patch();
            self.pre = semver::Prerelease::new(&format!("{}.1", VERSION_RC))?;
            Ok(())
        }
    }

    fn metadata(&mut self, build: &str) -> Result<(), FatalError> {
        self.build = semver::BuildMetadata::new(build)?;
        Ok(())
    }

    fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }
}

static VERSION_ALPHA: &str = "alpha";
static VERSION_BETA: &str = "beta";
static VERSION_RC: &str = "rc";

pub fn set_requirement(
    req: &semver::VersionReq,
    version: &semver::Version,
) -> Result<Option<String>, FatalError> {
    let req_text = req.to_string();
    let raw_req = semver::VersionReq::parse(&req_text)
        .expect("semver to generate valid version requirements");
    if raw_req.comparators.is_empty() {
        // Empty matches everything, no-change.
        Ok(None)
    } else {
        let comparators: Result<Vec<_>, _> = raw_req
            .comparators
            .into_iter()
            .map(|p| set_comparator(p, version))
            .collect();
        let comparators = comparators?;
        let new_req = semver::VersionReq { comparators };
        let new_req_text = new_req.to_string();
        // Validate contract
        #[cfg(debug_assert)]
        {
            assert!(
                new_req.matches(version),
                "Invalid req created: {}",
                new_req_text
            )
        }
        if new_req_text == req_text {
            Ok(None)
        } else {
            Ok(Some(new_req_text))
        }
    }
}

fn set_comparator(
    mut pred: semver::Comparator,
    version: &semver::Version,
) -> Result<semver::Comparator, FatalError> {
    match pred.op {
        semver::Op::Wildcard => {
            pred.major = version.major;
            if pred.minor.is_some() {
                pred.minor = Some(version.minor);
            }
            if pred.patch.is_some() {
                pred.patch = Some(version.patch);
            }
            Ok(pred)
        }
        semver::Op::Exact => assign_partial_req(version, pred),
        semver::Op::Greater | semver::Op::GreaterEq | semver::Op::Less | semver::Op::LessEq => {
            let user_pred = pred.to_string();
            Err(FatalError::UnsupportedVersionReq(user_pred))
        }
        semver::Op::Tilde => assign_partial_req(version, pred),
        semver::Op::Caret => assign_partial_req(version, pred),
        _ => {
            log::debug!("New predicate added");
            let user_pred = pred.to_string();
            Err(FatalError::UnsupportedVersionReq(user_pred))
        }
    }
}

fn assign_partial_req(
    version: &semver::Version,
    mut pred: semver::Comparator,
) -> Result<semver::Comparator, FatalError> {
    pred.major = version.major;
    if pred.minor.is_some() {
        pred.minor = Some(version.minor);
    }
    if pred.patch.is_some() {
        pred.patch = Some(version.patch);
    }
    pred.pre = version.pre.clone();
    Ok(pred)
}

#[cfg(test)]
mod test {
    use super::*;

    mod increment {
        use super::*;

        #[test]
        fn alpha() {
            let mut v = semver::Version::parse("1.0.0").unwrap();
            let _ = v.increment_alpha();
            assert_eq!(v, semver::Version::parse("1.0.1-alpha.1").unwrap());

            let mut v2 = semver::Version::parse("1.0.1-dev").unwrap();
            let _ = v2.increment_alpha();
            assert_eq!(v2, semver::Version::parse("1.0.1-alpha.1").unwrap());

            let mut v3 = semver::Version::parse("1.0.1-alpha.1").unwrap();
            let _ = v3.increment_alpha();
            assert_eq!(v3, semver::Version::parse("1.0.1-alpha.2").unwrap());

            let mut v4 = semver::Version::parse("1.0.1-beta.1").unwrap();
            assert!(v4.increment_alpha().is_err());
        }

        #[test]
        fn beta() {
            let mut v = semver::Version::parse("1.0.0").unwrap();
            let _ = v.increment_beta();
            assert_eq!(v, semver::Version::parse("1.0.1-beta.1").unwrap());

            let mut v2 = semver::Version::parse("1.0.1-dev").unwrap();
            let _ = v2.increment_beta();
            assert_eq!(v2, semver::Version::parse("1.0.1-beta.1").unwrap());

            let mut v2 = semver::Version::parse("1.0.1-alpha.1").unwrap();
            let _ = v2.increment_beta();
            assert_eq!(v2, semver::Version::parse("1.0.1-beta.1").unwrap());

            let mut v3 = semver::Version::parse("1.0.1-beta.1").unwrap();
            let _ = v3.increment_beta();
            assert_eq!(v3, semver::Version::parse("1.0.1-beta.2").unwrap());

            let mut v4 = semver::Version::parse("1.0.1-rc.1").unwrap();
            assert!(v4.increment_beta().is_err());
        }

        #[test]
        fn rc() {
            let mut v = semver::Version::parse("1.0.0").unwrap();
            let _ = v.increment_rc();
            assert_eq!(v, semver::Version::parse("1.0.1-rc.1").unwrap());

            let mut v2 = semver::Version::parse("1.0.1-dev").unwrap();
            let _ = v2.increment_rc();
            assert_eq!(v2, semver::Version::parse("1.0.1-rc.1").unwrap());

            let mut v3 = semver::Version::parse("1.0.1-rc.1").unwrap();
            let _ = v3.increment_rc();
            assert_eq!(v3, semver::Version::parse("1.0.1-rc.2").unwrap());
        }

        #[test]
        fn metadata() {
            let mut v = semver::Version::parse("1.0.0").unwrap();
            let _ = v.metadata("git.123456");
            assert_eq!(v, semver::Version::parse("1.0.0+git.123456").unwrap());
        }
    }

    mod set_requirement {
        use super::*;

        fn assert_req_bump<'a, O: Into<Option<&'a str>>>(version: &str, req: &str, expected: O) {
            let version = semver::Version::parse(version).unwrap();
            let req = semver::VersionReq::parse(req).unwrap();
            let actual = set_requirement(&req, &version).unwrap();
            let expected = expected.into();
            assert_eq!(actual.as_deref(), expected);
        }

        #[test]
        fn wildcard_major() {
            assert_req_bump("1.0.0", "*", None);
        }

        #[test]
        fn wildcard_minor() {
            assert_req_bump("1.0.0", "1.*", None);
            assert_req_bump("1.1.0", "1.*", None);
            assert_req_bump("2.0.0", "1.*", "2.*");
        }

        #[test]
        fn wildcard_patch() {
            assert_req_bump("1.0.0", "1.0.*", None);
            assert_req_bump("1.1.0", "1.0.*", "1.1.*");
            assert_req_bump("1.1.1", "1.0.*", "1.1.*");
            assert_req_bump("2.0.0", "1.0.*", "2.0.*");
        }

        #[test]
        fn caret_major() {
            assert_req_bump("1.0.0", "1", None);
            assert_req_bump("1.0.0", "^1", None);

            assert_req_bump("1.1.0", "1", None);
            assert_req_bump("1.1.0", "^1", None);

            assert_req_bump("2.0.0", "1", "^2");
            assert_req_bump("2.0.0", "^1", "^2");
        }

        #[test]
        fn caret_minor() {
            assert_req_bump("1.0.0", "1.0", None);
            assert_req_bump("1.0.0", "^1.0", None);

            assert_req_bump("1.1.0", "1.0", "^1.1");
            assert_req_bump("1.1.0", "^1.0", "^1.1");

            assert_req_bump("1.1.1", "1.0", "^1.1");
            assert_req_bump("1.1.1", "^1.0", "^1.1");

            assert_req_bump("2.0.0", "1.0", "^2.0");
            assert_req_bump("2.0.0", "^1.0", "^2.0");
        }

        #[test]
        fn caret_patch() {
            assert_req_bump("1.0.0", "1.0.0", None);
            assert_req_bump("1.0.0", "^1.0.0", None);

            assert_req_bump("1.1.0", "1.0.0", "^1.1.0");
            assert_req_bump("1.1.0", "^1.0.0", "^1.1.0");

            assert_req_bump("1.1.1", "1.0.0", "^1.1.1");
            assert_req_bump("1.1.1", "^1.0.0", "^1.1.1");

            assert_req_bump("2.0.0", "1.0.0", "^2.0.0");
            assert_req_bump("2.0.0", "^1.0.0", "^2.0.0");
        }

        #[test]
        fn tilde_major() {
            assert_req_bump("1.0.0", "~1", None);
            assert_req_bump("1.1.0", "~1", None);
            assert_req_bump("2.0.0", "~1", "~2");
        }

        #[test]
        fn tilde_minor() {
            assert_req_bump("1.0.0", "~1.0", None);
            assert_req_bump("1.1.0", "~1.0", "~1.1");
            assert_req_bump("1.1.1", "~1.0", "~1.1");
            assert_req_bump("2.0.0", "~1.0", "~2.0");
        }

        #[test]
        fn tilde_patch() {
            assert_req_bump("1.0.0", "~1.0.0", None);
            assert_req_bump("1.1.0", "~1.0.0", "~1.1.0");
            assert_req_bump("1.1.1", "~1.0.0", "~1.1.1");
            assert_req_bump("2.0.0", "~1.0.0", "~2.0.0");
        }

        #[test]
        fn equal_major() {
            assert_req_bump("1.0.0", "=1", None);
            assert_req_bump("1.1.0", "=1", None);
            assert_req_bump("2.0.0", "=1", "=2");
        }

        #[test]
        fn equal_minor() {
            assert_req_bump("1.0.0", "=1.0", None);
            assert_req_bump("1.1.0", "=1.0", "=1.1");
            assert_req_bump("1.1.1", "=1.0", "=1.1");
            assert_req_bump("2.0.0", "=1.0", "=2.0");
        }

        #[test]
        fn equal_patch() {
            assert_req_bump("1.0.0", "=1.0.0", None);
            assert_req_bump("1.1.0", "=1.0.0", "=1.1.0");
            assert_req_bump("1.1.1", "=1.0.0", "=1.1.1");
            assert_req_bump("2.0.0", "=1.0.0", "=2.0.0");
        }
    }
}
