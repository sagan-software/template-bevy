{
  description = "Bevy game template with optional reproducible tooling";

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
    dylints = {
      url = "github:sagan-software/dylints/9bc21efeccdd1236e3e64cf7bb607823c60d4600";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
        rust-overlay.follows = "rust-overlay";
        crane.follows = "crane";
        treefmt-nix.follows = "treefmt-nix";
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
      dylints,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        inherit (pkgs) lib;
        rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        craneLib = (crane.mkLib pkgs).overrideToolchain rust;
        appManifest = builtins.fromTOML (builtins.readFile ./crates/app/Cargo.toml);
        appName = appManifest.package.name;
        mcpEnabled = builtins.elem "mcp" (appManifest.features.default or [ ]);
        src = lib.cleanSourceWith {
          src = ./.;
          filter =
            path: type:
            lib.cleanSourceFilter path type
            && !(
              type == "directory"
              && builtins.elem (baseNameOf path) [
                "target"
                ".direnv"
                "book"
                "result"
                "template"
              ]
            );
        };
        native = with pkgs; [
          cmake
          clang
          lld
          makeWrapper
          pkg-config
        ];
        libraries = lib.optionals pkgs.stdenv.hostPlatform.isLinux (
          with pkgs;
          [
            alsa-lib
            libxkbcommon
            udev
            vulkan-loader
            wayland
            libx11
            libxcursor
            libxi
            libxrandr
          ]
        );
        environment = ''
          export PKG_CONFIG_PATH="${
            lib.makeSearchPathOutput "dev" "lib/pkgconfig" libraries
          }:''${PKG_CONFIG_PATH:-}"
          export LD_LIBRARY_PATH="${lib.makeLibraryPath libraries}:''${LD_LIBRARY_PATH:-}"
        '';
        common = {
          inherit src;
          strictDeps = true;
          nativeBuildInputs = native;
          buildInputs = libraries;
          pname = appName;
          version = "0.1.0";
        };
        artifacts = craneLib.buildDepsOnly common;
        package = craneLib.buildPackage (
          common
          // {
            cargoArtifacts = artifacts;
            cargoExtraArgs = "--locked -p ${appName}";
            doCheck = false;
            postFixup = lib.optionalString pkgs.stdenv.hostPlatform.isLinux ''
              wrapProgram "$out/bin/${appName}" --prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath libraries}"
            '';
          }
        );
        treefmt = treefmt-nix.lib.evalModule pkgs {
          projectRootFile = "flake.nix";
          programs.nixfmt.enable = true;
          programs.rustfmt.enable = true;
          settings.global.excludes = [
            ".direnv/**"
            ".git/**"
            "target/**"
            "docs/book/**"
            "result*/**"
          ];
        };
        lintCrane = (crane.mkLib pkgs).overrideToolchain lintToolchain;
        supported = builtins.hasAttr system dylints.packages;
        lintToolchain = pkgs.rust-bin.fromRustupToolchainFile "${dylints}/rust-toolchain.toml";
        lintBundle = dylints.packages.${system}.sagan-lints or null;
        lintMetadata =
          (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.metadata.dylint.libraries;
        lintGroups = lib.concatMap (
          entry:
          assert entry.git == "https://github.com/sagan-software/dylints" && entry.rev == dylints.rev;
          map builtins.baseNameOf entry.pattern
        ) lintMetadata;
        lintArgs = lib.concatMapStringsSep " " (
          group: "--dylint-category ${lib.escapeShellArg group}"
        ) lintGroups;
        command =
          name: inputs: text:
          pkgs.writeShellApplication {
            inherit name;
            runtimeInputs = [ rust ] ++ native ++ libraries ++ inputs;
            text = environment + text;
          };
        dylint = command "run-dylints" (lib.optionals supported [ lintBundle ]) (
          if supported then
            ''sagan-lints --repo . --skip-clippy --workspace --all-targets --locked ${lintArgs} "$@"''
          else
            ''echo "The pinned Dylints bundle does not support ${system}." >&2; exit 2''
        );
        bevyToolchain = pkgs.rust-bin.fromRustupToolchainFile "${bevy_cli}/rust-toolchain.toml";
        bevyCli = bevy_cli.packages.${system}.default.overrideAttrs (old: {
          buildInputs =
            (old.buildInputs or [ ]) ++ lib.optionals pkgs.stdenv.hostPlatform.isDarwin [ pkgs.zlib ];
          postInstall =
            (old.postInstall or "")
            + ''wrapProgram "$out/bin/bevy_lint" --prefix PATH : "${lib.makeBinPath [ bevyToolchain ]}"'';
        });
        mcp = pkgs.rustPlatform.buildRustPackage {
          pname = "bevy_brp_mcp";
          version = "0.22.8";
          src = pkgs.fetchCrate {
            pname = "bevy_brp_mcp";
            version = "0.22.8";
            hash = "sha256-X9We8y+VVfQB3HgDLMvZF+0Ismpf8qpvyGRfjpwt1uk=";
          };
          cargoHash = "sha256-M2xccmCc3eypqzTFQMAqRXpgk3NYYtQEN1ay+jeY020=";
          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = lib.optionals pkgs.stdenv.hostPlatform.isLinux [ pkgs.openssl ];
          doCheck = false;
          meta.mainProgram = "bevy_brp_mcp";
        };
        # Trunk's bundled libdeflate requires Clang with this nixpkgs compiler set.
        trunk = pkgs.trunk.overrideAttrs (old: {
          nativeBuildInputs = (old.nativeBuildInputs or [ ]) ++ [ pkgs.clang ];
          preBuild = (old.preBuild or "") + ''
            export CC_${
              lib.replaceStrings [ "-" ] [ "_" ] pkgs.stdenv.hostPlatform.rust.rustcTarget
            }="${pkgs.clang}/bin/clang"
          '';
        });
        tools =
          with pkgs;
          [
            cargo-generate
            cargo-llvm-cov
            cargo-nextest
            mdbook
            binaryen
            cargo-flamegraph
            samply
            hyperfine
          ]
          ++ [ trunk ]
          ++ lib.optionals pkgs.stdenv.hostPlatform.isLinux [ perf ];
        workflow = action: command "project-${action}" tools ''cargo xtask ${action} "$@"'';
        commands = {
          default = command "run-game" [ ] ''cargo run --locked -p ${lib.escapeShellArg appName} -- "$@"'';
          dev =
            command "run-dev" [ ]
              ''cargo run --locked -p ${lib.escapeShellArg appName} --features dev -- "$@"'';
          editor =
            command "run-editor" [ ]
              ''cargo run --locked -p ${lib.escapeShellArg appName} --features mcp -- --editor "$@"'';
          test = command "run-tests" [ ] ''cargo test --locked "$@"'';
          clippy = command "run-clippy" [ ] ''cargo clippy --all-targets --all-features -- -D warnings "$@"'';
          bench = command "run-benchmarks" [ ] ''cargo bench --workspace --locked -- "$@"'';
          fmt = command "format-project" [ treefmt.config.build.wrapper ] ''treefmt "$@"'';
          setup-ai = workflow "setup";
          book = workflow "book";
          coverage = workflow "coverage";
          features = workflow "features";
          generate-matrix = workflow "generate-matrix";
          bevy = command "run-bevy-cli" [ bevyCli ] ''bevy "$@"'';
          inherit dylint;
          check =
            command "check-project"
              (
                tools
                ++ [
                  dylint
                  treefmt.config.build.wrapper
                ]
              )
              ''
                treefmt --fail-on-change
                cargo xtask check
                run-dylints
              '';
        };
        gate =
          name: extra: script:
          craneLib.mkCargoDerivation (
            common
            // {
              cargoArtifacts = artifacts;
              pname = "${appName}-${name}";
              nativeBuildInputs = native ++ extra;
              buildPhaseCargoCommand = script;
              doInstallCargoArtifacts = false;
              installPhaseCommand = "mkdir -p $out";
            }
          );
        onboarding =
          pkgs.runCommand "dylint-onboarding"
            {
              nativeBuildInputs = [
                dylint
                lintToolchain
              ];
            }
            ''
              export CARGO_HOME="$TMPDIR/cargo" CARGO_NET_OFFLINE=true SAGAN_LINTS_CACHE_DIR="$TMPDIR/cache"
              mkdir -p repo/src
              cd repo
              cat > Cargo.toml <<'TOML'
              [package]
              name = "lint-onboarding"
              version = "0.1.0"
              edition = "2024"
              TOML
              echo 'pub fn count(values: Vec<u8>) -> usize { values.len() }' > src/lib.rs
              cargo generate-lockfile --offline
              status=0
              run-dylints > failing.log 2>&1 || status=$?
              cat failing.log
              test "$status" -eq 1
              grep -q ownership_at_boundaries failing.log
              echo 'pub fn count(values: &[u8]) -> usize { values.len() }' > src/lib.rs
              run-dylints
              touch "$out"
            '';
      in
      {
        packages = {
          default = package;
          "${appName}" = package;
          bevy-brp-mcp = mcp;
        };
        apps =
          lib.mapAttrs (
            name: drv:
            (flake-utils.lib.mkApp { inherit drv; })
            // {
              meta.description = "Run the project's ${name} workflow";
            }
          ) commands
          // {
            bevy-brp-mcp = (flake-utils.lib.mkApp { drv = mcp; }) // {
              meta.description = "Run the Bevy BRP MCP server";
            };
          };
        checks = {
          inherit package;
          fmt = treefmt.config.build.check src;
          clippy = gate "clippy" [ ] "cargo clippy --all-targets --all-features -- -D warnings";
          test = gate "test" [ ] "cargo test --locked";
          doctest = gate "doctest" [ ] "cargo test --workspace --doc --locked";
          private-docs =
            gate "private-docs" [ ]
              "RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --document-private-items --locked";
          bench = gate "bench" [ ] "cargo bench --workspace --locked --no-run";
          features = gate "features" [ ] "cargo xtask features";
          book = gate "book" [ pkgs.mdbook ] "mdbook build docs";
          coverage = gate "coverage" [
            pkgs.cargo-llvm-cov
          ] ''cargo xtask coverage; cp -r target/llvm-cov "$out"'';
        }
        // lib.optionalAttrs supported {
          dylint-onboarding = onboarding;
          dylint = lintCrane.mkCargoDerivation (
            common
            // {
              cargoArtifacts = lintCrane.buildDepsOnly common;
              nativeBuildInputs = native ++ [ lintBundle ];
              buildPhaseCargoCommand = ''
                # Keep the lint cache outside the source tree in writable sandbox storage.
                export SAGAN_LINTS_CACHE_DIR="$TMPDIR/sagan-lints"
                mkdir -p "$SAGAN_LINTS_CACHE_DIR"
                sagan-lints --repo . --skip-clippy --workspace --all-targets --locked ${lintArgs}
              '';
              doInstallCargoArtifacts = false;
              installPhaseCommand = "mkdir -p $out";
            }
          );
        };
        formatter = treefmt.config.build.wrapper;
        devShells.core = pkgs.mkShell {
          packages = [ rust ] ++ native ++ libraries;
          shellHook = environment;
        };
        devShells.default = pkgs.mkShell {
          packages = [
            rust
          ]
          ++ native
          ++ libraries
          ++ tools
          ++ lib.optionals mcpEnabled [ mcp ]
          ++ [ dylint ];
          shellHook = environment;
        };
      }
    );
}
