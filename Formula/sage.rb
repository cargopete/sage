class Sage < Formula
  desc "A programming language where agents are first-class citizens"
  homepage "https://github.com/sagelang/sage"
  version "1.0.4"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/sagelang/sage/releases/download/v1.0.4/sage-v1.0.4-aarch64-apple-darwin.tar.gz"
      sha256 "2c3b9b655060406f667018b7b9d015e27b92f644e7fe119b62b7860393f233fb"
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
          yield(42);
        }
      }
      run Main;
    EOS
    system "#{bin}/sage", "check", "hello.sg"
  end
end
