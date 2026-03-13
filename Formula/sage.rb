class Sage < Formula
  desc "A programming language where agents are first-class citizens"
  homepage "https://github.com/sagelang/sage"
  version "0.2.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/sagelang/sage/releases/download/v0.2.0/sage-v0.2.0-aarch64-apple-darwin.tar.gz"
      sha256 "e3de105016f48fb2872d26f600704042c3ec85d3e7a1ce74f40a41e9e9e52618"
    end
  end

  depends_on "openssl@3"

  def install
    bin.install "bin/sage"
    (share/"sage/toolchain").install Dir["toolchain/*"]
  end

  def caveats
    <<~EOS
      Add this to your shell profile for fast builds:
        export SAGE_TOOLCHAIN=#{opt_share}/sage/toolchain
    EOS
  end

  test do
    (testpath/"hello.sg").write <<~EOS
      agent Main {
        on start {
          emit(42);
        }
      }
      run Main;
    EOS
    system "#{bin}/sage", "check", "hello.sg"
  end
end
