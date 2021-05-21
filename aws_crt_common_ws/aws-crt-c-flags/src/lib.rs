use cc::Build;
use fs_extra;
use fs_extra::dir::CopyOptions;
use gag::Gag;
use serde::{Deserialize, Serialize};
use serde_json::Result;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CRTModuleBuildInfo {
    crt_module_name: String,
    crt_module_deps: Vec<CRTModuleBuildInfo>,
    private_cflags: Vec<String>,
    public_cflags: Vec<String>,
    private_defines: Vec<(String, String)>,
    public_defines: Vec<(String, String)>,
    link_targets: Vec<String>,
    shared_lib: bool,
    lib_name: String,
    linker_path: Option<PathBuf>,
    include_dirs: Vec<PathBuf>,
    #[serde(skip_serializing, skip_deserializing)]
    build_toolchain: Build,
}

impl CRTModuleBuildInfo {
    /// Creates a new instance of CRTModuleBuildInfo.
    /// # Arguments
    ///
    /// * `module_name` - Name of the module you want to build. This name will be used to identify
    ///                   build state across crates. We recommend you name it your sys crate name.
    ///                   For example: aws_crt_common_sys, aws_crt_http_sys etc...
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// use aws_crt_c_flags::{CRTModuleBuildInfo};
    /// let build_info = CRTModuleBuildInfo::new("aws_crt_common_sys");
    /// ```
    pub fn new(module_name: &str) -> CRTModuleBuildInfo {
        CRTModuleBuildInfo {
            crt_module_name: module_name.parse().unwrap(),
            crt_module_deps: vec![],
            private_cflags: vec![],
            public_cflags: vec![],
            private_defines: vec![],
            public_defines: vec![],
            link_targets: vec![],
            shared_lib: false,
            lib_name: module_name.parse().unwrap(),
            linker_path: Option::from(PathBuf::from(env::var_os("OUT_DIR").unwrap())),
            include_dirs: vec![],
            build_toolchain: Build::new(),
        }
    }

    /// Declares other aws crt modules to have your sys package depend on. This is used for transitively
    /// passing linker arguments and c-flags between different crates' builds of their sys modules.
    ///
    /// # Arguments
    ///
    /// * `dependency` - name of the crt sys crate you want to link your sys crate against. So for example,
    ///                  if you're building aws-checksums, your sys crate would be aws_crt_checksums_sys, and you
    ///                  would declare a dependency here on aws_crt_common_sys.
    ///
    /// # Examples
    /// ```should_panic
    /// use aws_crt_c_flags::{CRTModuleBuildInfo};
    /// let mut build_info = CRTModuleBuildInfo::new("aws_crt_checksums_sys");
    /// build_info.add_module_dependency("aws_crt_common_sys");
    /// ```
    pub fn add_module_dependency(&mut self, dependency: &str) -> &mut CRTModuleBuildInfo {
        let crt_module_env_var_res = env::var("CRT_MODULE_".to_owned() + dependency + "BUILD_CFG");

        if crt_module_env_var_res.is_ok() {
            let parse_res: Result<CRTModuleBuildInfo> =
                serde_json::from_str(crt_module_env_var_res.unwrap().as_str());

            if parse_res.is_ok() {
                self.crt_module_deps.push(parse_res.unwrap());
                return self;
            }
        }

        panic!("Module: `{}` does not appear to be a part of your dependency chain. Alternatively, the dependencies build script does not call CRTModuleBuildInfo::run_build() correctly", dependency);
    }

    /// Adds a c-flag to your build, and guarantees that when another module calls add_module_dependency()
    /// on the module_name you used in ::new(), it will be transitively applied to their build.
    ///
    /// # Arguments
    ///
    /// * `c_flag` - compiler flag to apply to the build. Example `-fPIC`
    ///
    /// # Examples
    /// ```should_panic
    /// use aws_crt_c_flags::{CRTModuleBuildInfo};
    /// let mut build_info = CRTModuleBuildInfo::new("aws_crt_common_sys");
    /// build_info.add_public_cflag("-fPIC");
    /// ```
    pub fn add_public_cflag(&mut self, c_flag: &str) -> &mut CRTModuleBuildInfo {
        self.public_cflags.push(c_flag.parse().unwrap());
        self
    }

    /// Adds a c-flag to your build, this flag will only apply to the current building module and
    /// will not be transitively reflected to other modules' builds.
    ///
    /// # Arguments
    ///
    /// * `c_flag` - compiler flag to apply to the build. Example `-fPIC`
    ///
    /// # Examples
    /// ```should_panic
    /// use aws_crt_c_flags::{CRTModuleBuildInfo};
    /// let mut build_info = CRTModuleBuildInfo::new("aws_crt_common_sys");
    /// build_info.add_private_cflag("-Wall");
    /// ```
    pub fn add_private_cflag(&mut self, c_flag: &str) -> &mut CRTModuleBuildInfo {
        self.private_cflags.push(c_flag.parse().unwrap());
        self
    }

    /// Adds a definition to your build, this flag will only apply to the current building module and
    /// will not be transitively reflected to other modules' builds.
    ///
    /// # Arguments
    ///
    /// * `key` - definition name
    /// * `val` - definition value
    ///
    /// this has the effect of writing #define [key] [value] inside your compilation units.
    ///
    /// # Examples
    /// ```should_panic
    /// use aws_crt_c_flags::{CRTModuleBuildInfo};
    /// let mut build_info = CRTModuleBuildInfo::new("aws_crt_common_sys");
    /// build_info.add_private_define("MY_DEFINE", "MY_DEFINE_VALUE");
    /// ```
    pub fn add_private_define(&mut self, key: &str, val: &str) -> &mut CRTModuleBuildInfo {
        self.private_defines
            .push((key.parse().unwrap(), val.parse().unwrap()));
        self
    }

    /// Adds a definition to your build and guarantees that when another module calls add_module_dependency()
    /// on the module_name you used in ::new(), it will be transitively applied to their build.
    ///
    /// # Arguments
    ///
    /// * `key` - definition name
    /// * `val` - definition value
    ///
    /// this has the effect of writing #define [key] [value] inside your compilation units.
    ///
    /// # Examples
    /// ```should_panic
    /// use aws_crt_c_flags::{CRTModuleBuildInfo};
    /// let mut build_info = CRTModuleBuildInfo::new("aws_crt_common_sys");
    /// build_info.add_public_define("MY_DEFINE", "MY_DEFINE_VALUE");
    /// ```
    pub fn add_public_define(&mut self, key: &str, val: &str) -> &mut CRTModuleBuildInfo {
        self.public_defines
            .push((key.parse().unwrap(), val.parse().unwrap()));
        self
    }

    /// Adds a library to the linker line. Formatting for things like framework-vs-system lib are the responsibility
    /// of the caller. All link targets are transitively passed to module consumers.
    ///
    /// # Arguments
    ///
    /// * `l_flag` - Linker target. Example: "curl", "Kernel32", "framework=CoreFoundation"
    ///
    /// # Examples
    /// ```should_panic
    /// use aws_crt_c_flags::{CRTModuleBuildInfo};
    /// let mut build_info = CRTModuleBuildInfo::new("aws_crt_common_sys");
    /// build_info.add_link_target("crypto")
    ///     .add_link_target("framework=Security");
    /// ```
    pub fn add_link_target(&mut self, l_flag: &str) -> &mut CRTModuleBuildInfo {
        self.link_targets.push(l_flag.parse().unwrap());
        self
    }

    /// Makes this module build as a shared library. The default is static.
    pub fn make_shared_lib(&mut self) -> &mut CRTModuleBuildInfo {
        self.shared_lib = true;
        self
    }

    /// Sets the linker search path. Currently this is unused, but if you were to be using this
    /// module to link against a library built via. something other than cargo, you'd use this to do so.
    /// The default is the cargo build output directory.
    ///
    /// # Arguments
    ///
    /// * `path` - File system path to where to find libraries to link against (e.g. /usr/lib)
    ///
    /// # Examples
    /// ```should_panic
    /// use aws_crt_c_flags::{CRTModuleBuildInfo};
    /// use std::path::Path;
    /// let mut build_info = CRTModuleBuildInfo::new("aws_crt_common_sys");
    /// build_info.set_linker_search_path(Path::new("/opt/where/i/installed/my/manually/built/libcrypto/lib"))
    ///     .add_link_target("crypto");
    /// ```
    pub fn set_linker_search_path(&mut self, path: &Path) -> &mut CRTModuleBuildInfo {
        self.linker_path = Some(path.to_path_buf());
        self
    }

    /// adds an additional include directory to your module build. This is mainly useful only
    /// if you're using a 3rd party library, not built by this library, and you need the compiler
    /// to fine the header files.
    ///
    /// # Arguments
    ///
    /// * `dir` - File system path to where to find the header files you need.
    ///
    /// # Examples
    /// ```should_panic
    /// use aws_crt_c_flags::{CRTModuleBuildInfo};
    /// use std::path::Path;
    /// let mut build_info = CRTModuleBuildInfo::new("aws_crt_common_sys");
    /// build_info.add_third_party_include_dir(Path::new("/opt/where/i/installed/my/manually/built/libcrypto/include"))
    ///     .set_linker_search_path(Path::new("/opt/where/i/installed/my/manually/built/libcrypto/lib"))
    ///     .add_link_target("crypto");
    /// ```
    pub fn add_third_party_include_dir(&mut self, dir: &Path) -> &mut CRTModuleBuildInfo {
        self.include_dirs.push(dir.to_path_buf());
        self
    }

    /// Adds an include directory from you one part of your source, to the build's closure so the header
    /// files can be accessed across builds.
    ///
    /// # Arguments
    ///
    /// * `dir` - File system path to the files you want copied to your build output and added to your
    ///           build for inclusion.
    ///
    /// # Examples
    /// ```should_panic
    /// use aws_crt_c_flags::{CRTModuleBuildInfo};
    /// use std::path::Path;
    /// let mut build_info = CRTModuleBuildInfo::new("aws_crt_common_sys");
    /// build_info.add_include_dir_and_copy_to_build_tree(Path::new("my_c_project/include"));
    /// ```
    pub fn add_include_dir_and_copy_to_build_tree(
        &mut self,
        dir: &Path,
    ) -> &mut CRTModuleBuildInfo {
        let out_dir = format!(
            "{}/include",
            env::var_os("OUT_DIR").unwrap().to_str().unwrap()
        );
        let target_include_path = Path::new(out_dir.as_str());
        fs::create_dir_all(target_include_path).expect("Creation of directory failed!");

        let mut terrible_api_hack = vec![];
        terrible_api_hack.push(dir);

        let mut copy_options = CopyOptions::new();
        copy_options.overwrite = true;

        fs_extra::copy_items(
            terrible_api_hack.as_ref(),
            target_include_path,
            &copy_options,
        )
        .expect("Copy failed, check the directory exists");
        self.include_dirs.push(target_include_path.to_path_buf());

        self
    }

    /// Writes generated content to your build directory, for use with things like autoconf output.
    ///
    /// # Arguments
    ///
    /// * `generated_content` - The file contents you want to write.
    /// * `to` - Location relative to your build tree you'd like to write to.
    ///
    ///# Examples
    ///```should_panic
    /// use aws_crt_c_flags::{CRTModuleBuildInfo};
    /// use std::path::Path;
    /// let mut build_info = CRTModuleBuildInfo::new("aws_crt_common_sys");
    /// let content = "test content".to_string();
    /// build_info.write_generated_file_to_output_path(&content, Path::new("include/aws/common/config.h"));
    ///```
    pub fn write_generated_file_to_output_path(
        &mut self,
        generated_content: &String,
        to: &Path,
    ) -> &mut CRTModuleBuildInfo {
        let target_location: String = format!(
            "{}/{}",
            env::var_os("OUT_DIR").unwrap().to_str().unwrap(),
            to.display()
        );
        let target_path = Path::new(target_location.as_str());
        fs::create_dir_all(target_path.parent().unwrap())
            .expect("Creation of output directory failed.");
        fs::write(target_location, generated_content).expect("Writing generated file failed!");

        self
    }

    /// Returns the underlying toolchain that will be used for the build.
    pub fn get_toolchain(&self) -> &Build {
        &self.build_toolchain
    }

    /// Adds the file at path to the build tree
    pub fn add_file_to_build(&mut self, path: &Path) -> &mut CRTModuleBuildInfo {
        self.build_toolchain.file(path);
        self
    }

    /// Attempts to compile, `to_compile` and returns a result on whether or not it succeeded.
    /// This is useful for testing compiler capabilities before including a file or flag in your build.
    ///
    /// # Arguments
    ///
    /// * `to_compile` - C code to attempt compilation of.
    ///
    /// # Examples
    /// ```should_panic
    /// use aws_crt_c_flags::{CRTModuleBuildInfo};
    /// let mut build_info = CRTModuleBuildInfo::new("aws_crt_common_sys");
    /// build_info.try_compile("int main() { return 0; }").expect("This should have compiled");
    /// ```
    pub fn try_compile(&self, to_compile: &str) -> core::result::Result<(), cc::Error> {
        // try_compile prints linker stuff. We don't want that since this is just for testing the
        // compiler capabilities. Suppress it for this scope.
        let _suppress_stdout_cause_the_build_prints_linker_nonsense = Gag::stdout().unwrap();
        let mut test_build = Build::new();
        let output_location = format!(
            "{}/compiler_checks",
            env::var_os("OUT_DIR").unwrap().to_str().unwrap()
        );
        test_build.out_dir(&output_location);
        fs::create_dir_all(&output_location).expect("creation of try compile directory failed");
        let target_location = format!(
            "{}/compiler_checks/check.c",
            env::var_os("OUT_DIR").unwrap().to_str().unwrap()
        );
        fs::write(Path::new(&target_location.as_str()), to_compile).expect("File write failed");
        test_build.file(&target_location);
        let res = test_build.try_compile("test");
        fs::remove_dir_all(&output_location).expect("Cleanup of try compile step failed!");
        res
    }

    fn load_to_build(&mut self) {
        // add default warning stuff.
        if self.build_toolchain.get_compiler().is_like_msvc() {
            self.add_private_cflag("/W4")
                .add_private_cflag("/WX")
                .add_private_cflag("/MP");
            // relaxes some implicit memory barriers that MSVC normally applies for volatile accesses
            self.add_private_cflag("/volatile:iso");
            // disable non-constant initializer warning, it's not non-standard, just for Microsoft
            self.add_private_cflag("/wd4204");
            // disable passing the address of a local warning. Again, not non-standard, just for Microsoft
            self.add_private_cflag("/wd4221");
        } else {
            self.add_private_cflag("-Wall")
                .add_private_cflag("-Werror")
                .add_private_cflag("-Wstrict-prototypes")
                .add_private_cflag("-fno-omit-frame-pointer")
                .add_private_cflag("-Wextra")
                .add_private_cflag("-pedantic")
                .add_private_cflag("-Wno-long-long")
                .add_private_cflag("-fPIC");
        }

        if self.build_toolchain.is_flag_supported("-Wgnu").is_ok() {
            // -Wgnu-zero-variadic-macro-arguments results in a lot of false positives
            self.add_private_cflag("-Wgnu")
                .add_private_cflag("-Wno-gnu-zero-variadic-macro-arguments");

            if self
                .try_compile(
                    " #include <netinet/in.h>
            int main() {
            uint32_t x = 0;
            x = htonl(x);
            return (int)x;
            }",
                )
                .is_err()
            {
                self.add_private_cflag("-Wno-gnu-statement-expression");
            }
        }

        for pub_flag in &self.public_cflags {
            self.build_toolchain.flag_if_supported(pub_flag.as_str());
        }

        for priv_flag in &self.private_cflags {
            self.build_toolchain.flag_if_supported(priv_flag.as_str());
        }

        for pub_define in &self.public_defines {
            self.build_toolchain
                .define(pub_define.0.as_str(), pub_define.1.as_str());
        }

        for priv_define in &self.private_defines {
            self.build_toolchain
                .define(priv_define.0.as_str(), priv_define.1.as_str());
        }

        for include in &self.include_dirs {
            self.build_toolchain.include(include);
        }

        for module in &self.crt_module_deps {
            for pub_flag in &module.public_cflags {
                self.build_toolchain.flag(pub_flag.as_str());
            }

            for pub_define in &self.public_defines {
                self.build_toolchain
                    .define(pub_define.0.as_str(), pub_define.1.as_str());
            }

            for include in &self.include_dirs {
                self.build_toolchain.include(include);
            }
        }

        if self.shared_lib {
            self.build_toolchain.shared_flag(true);
        }
    }

    /// Executes the build and if successful stores this object in the environment for the next crate to use.
    pub fn run_build(&mut self) {
        self.load_to_build();
        print!("{}", serde_json::to_string(self).unwrap().as_str());
        self.build_toolchain.compile(self.lib_name.as_str());

        if self.linker_path.is_some() {
            println!(
                "cargo:rustc-link-search={}",
                self.linker_path.as_ref().unwrap().to_str().unwrap()
            );
        }

        for link_flag in &self.link_targets {
            println!("cargo:rustc-link-lib={}", link_flag)
        }

        let module_name_cpy = self.crt_module_name.clone();
        env::set_var(
            "CRT_MODULE_".to_owned() + module_name_cpy.as_str() + "BUILD_CFG",
            serde_json::to_string(self).unwrap(),
        );
    }
}