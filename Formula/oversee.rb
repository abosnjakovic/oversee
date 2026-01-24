class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.2.6"

  if Hardware::CPU.arm?
    url "https://github.com/abosnjakovic/oversee/releases/download/v0.2.6/oversee-0.2.6-aarch64-apple-darwin.tar.gz"
    sha256 "23c21d3c92684ca7c1e1d6af42d66f57a7a2ef2ead594db1095e797e9162cc1f"
  else
    url "https://github.com/abosnjakovic/oversee/releases/download/v0.2.6/oversee-0.2.6-x86_64-apple-darwin.tar.gz"
    sha256 "b4dbe5c419389df35a7380c998f95a772088b0514fd252b37a6cb2453b03a46b"
  end

  def install
    bin.install "oversee"
  end

  test do
    system "#{bin}/oversee", "--version"
  end
end
