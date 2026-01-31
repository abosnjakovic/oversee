class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.2.7"

  if Hardware::CPU.arm?
    url "https://github.com/abosnjakovic/oversee/releases/download/v0.2.7/oversee-0.2.7-aarch64-apple-darwin.tar.gz"
    sha256 "8f8abd7933422f40a59d534c5b301c3302a1ec111fb543d9d74c909a90f44bad"
  else
    url "https://github.com/abosnjakovic/oversee/releases/download/v0.2.7/oversee-0.2.7-x86_64-apple-darwin.tar.gz"
    sha256 "3f7d2fcce1918c0981d1339a774045925483d9fe887b34aaf0bcb3c324618ae8"
  end

  def install
    bin.install "oversee"
  end

  test do
    system "#{bin}/oversee", "--version"
  end
end
