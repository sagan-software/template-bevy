{
  description = "Bevy game template with Nix-first workflows";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane = {
      url = "github:ipetkov/crane";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    bevy_cli = {
      url = "github:TheBevyFlock/bevy_cli";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
        rust-overlay.follows = "rust-overlay";
      };
    };
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      treefmt-nix,
      crane,
      rust-overlay,
      bevy_cli,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        bevyLintToolchain = pkgs.rust-bin.fromRustupToolchainFile "${bevy_cli}/rust-toolchain.toml";
        bevyCli = bevy_cli.packages.${system}.default.overrideAttrs (oldAttrs: {
          buildInputs =
            (oldAttrs.buildInputs or [ ]) ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [ pkgs.zlib ];
          postInstall = (oldAttrs.postInstall or "") + ''
            wrapProgram "$out/bin/bevy_lint" \
              --prefix PATH : "${pkgs.lib.makeBinPath [ bevyLintToolchain ]}"
          '';
        });
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        src = craneLib.cleanCargoSource ./.;
        packageName = "template-bevy";
        coverageThreshold = 50;
        coverageIgnoreRegex = "(^|/)(tests|benches)/";
        supportedFeatures = [
          "dev"
          "dynamic_linking"
          "trace_chrome"
          "trace_tracy"
        ];
        supportedFeatureCheckCommands = pkgs.lib.concatMapStringsSep "\n" (
          feature: "cargo check --workspace --locked --no-default-features --features '${feature}'"
        ) supportedFeatures;

        bevyNativeBuildInputs = [
          pkgs.cmake
          pkgs.clang
          pkgs.lld
          pkgs.makeWrapper
          pkgs.pkg-config
        ];

        bevyBuildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux [
          pkgs.alsa-lib
          pkgs.libxkbcommon
          pkgs.udev
          pkgs.vulkan-loader
          pkgs.wayland
          pkgs.libx11
          pkgs.libxcursor
          pkgs.libxi
          pkgs.libxrandr
        ];

        runtimeLibraryPath = pkgs.lib.makeLibraryPath bevyBuildInputs;
        pkgConfigPath = pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" bevyBuildInputs;

        bevyBrpMcp =
          let
            pname = "bevy_brp_mcp";
            version = "0.20.1";
          in
          pkgs.rustPlatform.buildRustPackage {
            inherit pname version;

            src = pkgs.fetchCrate {
              inherit pname version;
              hash = "sha256-pFE8vKDwuc9e8viKlidPRnvdC5JlF90/vgApzvJXLyQ=";
            };

            cargoHash = "sha256-rDjhWN1Sc+D0Oi5rZuL80Ifa1BU8dvJlAFYjfINYPDo=";
            doCheck = false;

            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.openssl ];

            meta.mainProgram = "bevy_brp_mcp";
          };

        aiSupport = pkgs.callPackage ./ai/default.nix {
          inherit bevyBrpMcp packageName;
        };

        commonArgs = {
          inherit src;
          strictDeps = true;
          nativeBuildInputs = bevyNativeBuildInputs;
          buildInputs = bevyBuildInputs;
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        templateBevy = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            pname = packageName;
            version = "0.1.0";
            doCheck = false;
            postFixup = pkgs.lib.optionalString pkgs.stdenv.isLinux ''
              wrapProgram "$out/bin/${packageName}" \
                --prefix LD_LIBRARY_PATH : "${runtimeLibraryPath}"
            '';
          }
        );

        ensureAgentLink = aiSupport.ensureLinks;

        withAgentLink =
          text:
          ''
            set -euo pipefail
            ensure-ai-links
          ''
          + text;

        # dprint accepts local WASM paths: https://dprint.dev/config/#plugins.
        # Fetch pinned plugins before the network-disabled formatting check.
        dprintSettings = builtins.fromJSON (builtins.readFile ./dprint.json);
        dprintPluginHashes = builtins.fromJSON (builtins.readFile ./nix/dprint-plugin-hashes.json);
        dprintOfflineConfig = pkgs.writeText "dprint-offline.json" (
          builtins.toJSON (
            dprintSettings
            // {
              plugins = map (
                url:
                toString (
                  pkgs.fetchurl {
                    inherit url;
                    sha256 = dprintPluginHashes.${url};
                  }
                )
              ) dprintSettings.plugins;
            }
          )
        );

        treefmtEval = treefmt-nix.lib.evalModule pkgs {
          projectRootFile = "flake.nix";
          programs = {
            dprint.enable = true;
            nixfmt.enable = true;
            rustfmt.enable = true;
          };
          settings.global.excludes = [
            ".direnv/**"
            ".git/**"
            "target/**"
            "result*/**"
          ];
          settings.formatter.dprint.options = [
            "--allow-no-files"
            "--config"
            (toString dprintOfflineConfig)
          ];
        };

        treefmtCheckEval = treefmt-nix.lib.evalModule pkgs {
          projectRootFile = "flake.nix";
          programs = {
            dprint.enable = true;
            nixfmt.enable = true;
            rustfmt.enable = true;
          };
          settings.global.excludes = [
            ".direnv/**"
            ".git/**"
            "target/**"
            "result*/**"
          ];
          settings.formatter.dprint.options = [
            "--allow-no-files"
            "--config"
            (toString dprintOfflineConfig)
          ];
        };

        formatRepo = pkgs.writeShellApplication {
          name = "format-repo";
          runtimeInputs = [
            ensureAgentLink
            treefmtEval.config.build.wrapper
          ];
          text = withAgentLink ''
            treefmt
          '';
        };

        formatCheck =
          pkgs.runCommand "format-check"
            {
              nativeBuildInputs = [ treefmtCheckEval.config.build.wrapper ];
            }
            ''
              cp -r ${./.} ./repo
              chmod -R +w ./repo
              cd ./repo

              export HOME="$TMPDIR"
              export XDG_CACHE_HOME="$TMPDIR/.cache"
              treefmt --fail-on-change

              touch "$out"
            '';

        clippyCheck = craneLib.cargoClippy (
          commonArgs
          // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--workspace --all-targets -- --deny warnings";
          }
        );

        testCheck = craneLib.cargoNextest (
          commonArgs
          // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
            cargoNextestExtraArgs = "--workspace --no-tests=pass";
          }
        );

        doctestCheck = craneLib.mkCargoDerivation (
          commonArgs
          // {
            inherit cargoArtifacts;
            pname = "${packageName}-doctest";
            version = "0.1.0";
            buildPhaseCargoCommand = "cargo test --workspace --doc --locked";
            doInstallCargoArtifacts = false;
            installPhaseCommand = "mkdir -p $out";
          }
        );

        privateDocsCheck = craneLib.mkCargoDerivation (
          commonArgs
          // {
            inherit cargoArtifacts;
            pname = "${packageName}-private-docs";
            version = "0.1.0";
            RUSTDOCFLAGS = "-D warnings";
            buildPhaseCargoCommand = "cargo doc --workspace --no-deps --document-private-items --locked";
            doInstallCargoArtifacts = false;
            installPhaseCommand = "mkdir -p $out";
          }
        );

        benchmarkCheck = craneLib.mkCargoDerivation (
          commonArgs
          // {
            inherit cargoArtifacts;
            pname = "${packageName}-bench-check";
            version = "0.1.0";
            buildPhaseCargoCommand = "cargo bench --workspace --locked --no-run";
            doInstallCargoArtifacts = false;
            installPhaseCommand = "mkdir -p $out";
          }
        );

        featureMatrixCheck = craneLib.mkCargoDerivation (
          commonArgs
          // {
            inherit cargoArtifacts;
            pname = "${packageName}-feature-matrix-check";
            version = "0.1.0";
            buildPhaseCargoCommand = supportedFeatureCheckCommands;
            doInstallCargoArtifacts = false;
            installPhaseCommand = "mkdir -p $out";
          }
        );

        coverageReport = craneLib.mkCargoDerivation (
          commonArgs
          // {
            inherit cargoArtifacts;
            pname = "${packageName}-coverage";
            version = "0.1.0";
            nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ pkgs.cargo-llvm-cov ];
            buildPhaseCargoCommand = ''
              mkdir -p "$out"
              cargo llvm-cov clean --workspace
              cargo llvm-cov --workspace --locked --remap-path-prefix --no-report
              cargo llvm-cov report --html --output-dir "$out" \
                --ignore-filename-regex '${coverageIgnoreRegex}' \
                --remap-path-prefix
              cargo llvm-cov report --lcov --output-path "$out/lcov.info" \
                --ignore-filename-regex '${coverageIgnoreRegex}' \
                --remap-path-prefix
              cargo llvm-cov report --json --output-path "$out/coverage.json" \
                --ignore-filename-regex '${coverageIgnoreRegex}' \
                --skip-functions \
                --remap-path-prefix
              cargo llvm-cov report \
                --fail-under-lines ${toString coverageThreshold} \
                --ignore-filename-regex '${coverageIgnoreRegex}' \
                --show-missing-lines \
                --remap-path-prefix
              test -s "$out/html/index.html"
              test -s "$out/lcov.info"
              test -s "$out/coverage.json"
            '';
            doInstallCargoArtifacts = false;
            installPhaseCommand = "true";
          }
        );

        bevyLintCheck = craneLib.mkCargoDerivation (
          commonArgs
          // {
            cargoArtifacts = null;
            pname = "${packageName}-bevy-lint";
            version = "0.1.0";
            nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ bevyCli ];
            buildPhaseCargoCommand = ''
              CARGO_TARGET_DIR=target/bevy-lint bevy_lint --workspace --all-targets --locked
            '';
            doInstallCargoArtifacts = false;
            installPhaseCommand = "mkdir -p $out";
          }
        );

        runCoverage = pkgs.writeShellApplication {
          name = "run-coverage";
          runtimeInputs = [
            ensureAgentLink
            pkgs.cargo-llvm-cov
            rustToolchain
          ]
          ++ bevyNativeBuildInputs
          ++ bevyBuildInputs;
          text = withAgentLink ''
            export PKG_CONFIG_PATH="${pkgConfigPath}:''${PKG_CONFIG_PATH:-}"
            export LD_LIBRARY_PATH="${runtimeLibraryPath}:''${LD_LIBRARY_PATH:-}"

            report_dir="target/llvm-cov"
            mkdir -p "$report_dir"

            cargo llvm-cov clean --workspace
            cargo llvm-cov --workspace --locked --remap-path-prefix --no-report
            cargo llvm-cov report --html --output-dir "$report_dir" \
              --ignore-filename-regex '${coverageIgnoreRegex}' \
              --remap-path-prefix
            cargo llvm-cov report --lcov --output-path "$report_dir/lcov.info" \
              --ignore-filename-regex '${coverageIgnoreRegex}' \
              --remap-path-prefix
            cargo llvm-cov report --json --output-path "$report_dir/coverage.json" \
              --ignore-filename-regex '${coverageIgnoreRegex}' \
              --skip-functions \
              --remap-path-prefix
            cargo llvm-cov report \
              --fail-under-lines ${toString coverageThreshold} \
              --ignore-filename-regex '${coverageIgnoreRegex}' \
              --show-missing-lines \
              --remap-path-prefix

            test -s "$report_dir/html/index.html"
            test -s "$report_dir/lcov.info"
            test -s "$report_dir/coverage.json"
          '';
        };

        runChecks = pkgs.writeShellApplication {
          name = "run-checks";
          runtimeInputs = [
            ensureAgentLink
            treefmtEval.config.build.wrapper
            pkgs.cargo-nextest
            rustToolchain
            bevyCli
            runCoverage
          ]
          ++ bevyNativeBuildInputs
          ++ bevyBuildInputs;
          text = withAgentLink ''
            export PKG_CONFIG_PATH="${pkgConfigPath}:''${PKG_CONFIG_PATH:-}"
            export LD_LIBRARY_PATH="${runtimeLibraryPath}:''${LD_LIBRARY_PATH:-}"
            treefmt --fail-on-change
            cargo clippy --workspace --all-targets --locked -- --deny warnings
            ${supportedFeatureCheckCommands}
            CARGO_TARGET_DIR=target/bevy-lint bevy_lint --workspace --all-targets --locked
            cargo nextest run --locked --workspace --no-tests=pass
            cargo test --workspace --doc --locked
            RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items --locked
            cargo bench --workspace --locked --no-run
            run-coverage
          '';
        };

        runClippy = pkgs.writeShellApplication {
          name = "run-clippy";
          runtimeInputs = [
            ensureAgentLink
            rustToolchain
          ]
          ++ bevyNativeBuildInputs
          ++ bevyBuildInputs;
          text = withAgentLink ''
            export PKG_CONFIG_PATH="${pkgConfigPath}:''${PKG_CONFIG_PATH:-}"
            export LD_LIBRARY_PATH="${runtimeLibraryPath}:''${LD_LIBRARY_PATH:-}"
            cargo clippy --workspace --all-targets --locked -- --deny warnings
          '';
        };

        runFeatureChecks = pkgs.writeShellApplication {
          name = "check-supported-features";
          runtimeInputs = [
            ensureAgentLink
            rustToolchain
          ]
          ++ bevyNativeBuildInputs
          ++ bevyBuildInputs;
          text = withAgentLink ''
            export PKG_CONFIG_PATH="${pkgConfigPath}:''${PKG_CONFIG_PATH:-}"
            export LD_LIBRARY_PATH="${runtimeLibraryPath}:''${LD_LIBRARY_PATH:-}"
            ${supportedFeatureCheckCommands}
          '';
        };

        runTests = pkgs.writeShellApplication {
          name = "run-tests";
          runtimeInputs = [
            ensureAgentLink
            pkgs.cargo-nextest
            rustToolchain
          ]
          ++ bevyNativeBuildInputs
          ++ bevyBuildInputs;
          text = withAgentLink ''
            export PKG_CONFIG_PATH="${pkgConfigPath}:''${PKG_CONFIG_PATH:-}"
            export LD_LIBRARY_PATH="${runtimeLibraryPath}:''${LD_LIBRARY_PATH:-}"
            cargo nextest run --locked --workspace --no-tests=pass
            cargo test --workspace --doc --locked
          '';
        };

        runBenchmarks = pkgs.writeShellApplication {
          name = "run-benchmarks";
          runtimeInputs = [
            ensureAgentLink
            rustToolchain
          ]
          ++ bevyNativeBuildInputs
          ++ bevyBuildInputs;
          text = withAgentLink ''
            export PKG_CONFIG_PATH="${pkgConfigPath}:''${PKG_CONFIG_PATH:-}"
            export LD_LIBRARY_PATH="${runtimeLibraryPath}:''${LD_LIBRARY_PATH:-}"
            cargo bench --workspace --locked -- "$@"
          '';
        };

        runDefault = pkgs.writeShellApplication {
          name = "run-${packageName}";
          runtimeInputs = [
            ensureAgentLink
            rustToolchain
          ]
          ++ bevyNativeBuildInputs
          ++ bevyBuildInputs;
          text = withAgentLink ''
            export PKG_CONFIG_PATH="${pkgConfigPath}:''${PKG_CONFIG_PATH:-}"
            export LD_LIBRARY_PATH="${runtimeLibraryPath}:''${LD_LIBRARY_PATH:-}"
            cargo run --locked -- "$@"
          '';
        };

        runDev = pkgs.writeShellApplication {
          name = "run-${packageName}-dev";
          runtimeInputs = [
            ensureAgentLink
            rustToolchain
          ]
          ++ bevyNativeBuildInputs
          ++ bevyBuildInputs;
          text = withAgentLink ''
            export PKG_CONFIG_PATH="${pkgConfigPath}:''${PKG_CONFIG_PATH:-}"
            export LD_LIBRARY_PATH="${runtimeLibraryPath}:''${LD_LIBRARY_PATH:-}"
            cargo run --locked --features dev -- "$@"
          '';
        };

        runEditor = pkgs.writeShellApplication {
          name = "run-${packageName}-editor";
          runtimeInputs = [
            ensureAgentLink
            rustToolchain
          ]
          ++ bevyNativeBuildInputs
          ++ bevyBuildInputs;
          text = withAgentLink ''
            export PKG_CONFIG_PATH="${pkgConfigPath}:''${PKG_CONFIG_PATH:-}"
            export LD_LIBRARY_PATH="${runtimeLibraryPath}:''${LD_LIBRARY_PATH:-}"
            cargo run --locked -- --editor "$@"
          '';
        };
      in
      {
        packages = {
          default = templateBevy;
          bevy-brp-mcp = bevyBrpMcp;
          coverage-report = coverageReport;
          "${packageName}" = templateBevy;
        };

        apps =
          pkgs.lib.mapAttrs (name: app: app // { meta.description = "Bevy template ${name} command"; })
            {
              default = flake-utils.lib.mkApp { drv = runDefault; };
              bench = flake-utils.lib.mkApp { drv = runBenchmarks; };
              bevy-brp-mcp = flake-utils.lib.mkApp { drv = bevyBrpMcp; };
              coverage = flake-utils.lib.mkApp { drv = runCoverage; };
              dev = flake-utils.lib.mkApp { drv = runDev; };
              editor = flake-utils.lib.mkApp { drv = runEditor; };
              features = flake-utils.lib.mkApp { drv = runFeatureChecks; };
              fmt = flake-utils.lib.mkApp { drv = formatRepo; };
              setup-ai = flake-utils.lib.mkApp { drv = ensureAgentLink; };
              check = flake-utils.lib.mkApp { drv = runChecks; };
              clippy = flake-utils.lib.mkApp { drv = runClippy; };
              test = flake-utils.lib.mkApp { drv = runTests; };
            };

        checks = {
          fmt = formatCheck;
          clippy = clippyCheck;
          bevy-lint = bevyLintCheck;
          bench = benchmarkCheck;
          coverage = coverageReport;
          doctest = doctestCheck;
          features = featureMatrixCheck;
          private-docs = privateDocsCheck;
          test = testCheck;
          package = templateBevy;
        };

        formatter = formatRepo;

        devShells.default = pkgs.mkShell {
          packages = [
            bevyBrpMcp
            bevyCli
            pkgs.cmake
            pkgs.clang
            pkgs.cargo-llvm-cov
            pkgs.cargo-nextest
            ensureAgentLink
            pkgs.lld
            pkgs.pkg-config
            treefmtEval.config.build.wrapper
            rustToolchain
          ]
          ++ bevyBuildInputs;
          shellHook = ''
            ${aiSupport.shellHook}
            export PKG_CONFIG_PATH="${pkgConfigPath}:''${PKG_CONFIG_PATH:-}"
            export LD_LIBRARY_PATH="${runtimeLibraryPath}:''${LD_LIBRARY_PATH:-}"
          '';
        };
      }
    );
}
