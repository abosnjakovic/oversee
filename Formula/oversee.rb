class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.3.5"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.5/oversee-0.3.5-aarch64-apple-darwin.tar.gz"
      sha256 "aa69148c20dc73a07ea707b3a24bca5a631ba610058afd7ad6e6787079e83696"
    else
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.5/oversee-0.3.5-x86_64-apple-darwin.tar.gz"
      sha256 "51d027b0007c3e5d9cf39f09d7ab2cee2973d40925b2e4dbffe21ca71252119d"
    end
  end

  def install
    bin.install "oversee"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/oversee --version")
  end
end
