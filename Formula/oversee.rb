class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.3.6"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.6/oversee-0.3.6-aarch64-apple-darwin.tar.gz"
      sha256 "a492dd2deca9f5140717ac2dbb3204a432645c0751b2d9b69da284ff9f5f4368"
    else
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.6/oversee-0.3.6-x86_64-apple-darwin.tar.gz"
      sha256 "0679684275265eadbceac046d8352e8923d481f0533dc1c5a19ec3dcd48a8fd0"
    end
  end

  def install
    bin.install "oversee"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/oversee --version")
  end
end
