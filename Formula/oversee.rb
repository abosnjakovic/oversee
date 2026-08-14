class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.3.8"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.8/oversee-0.3.8-aarch64-apple-darwin.tar.gz"
      sha256 "454f4c60a9f6a1cb6e9b5ad2785e5dafe0c004c4b734a6ae639affadf1bda90b"
    else
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.8/oversee-0.3.8-x86_64-apple-darwin.tar.gz"
      sha256 "f3bb76418d847bc54917d1ad071221c071986d041dec6f36149d3f096faccc52"
    end
  end

  def install
    bin.install "oversee"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/oversee --version")
  end
end
