class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.3.7"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.7/oversee-0.3.7-aarch64-apple-darwin.tar.gz"
      sha256 "29d87d2ff1d827387aaa1d8e47cbebd5b4a4684e015474bd532cc3ca6a624437"
    else
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.7/oversee-0.3.7-x86_64-apple-darwin.tar.gz"
      sha256 "2a4dc01e239f69a997fbd01f14163bc9983b03615ea3fb25fca954501314aa4f"
    end
  end

  def install
    bin.install "oversee"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/oversee --version")
  end
end
