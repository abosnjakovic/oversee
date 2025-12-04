class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.2.0"

  if Hardware::CPU.arm?
    url "https://github.com/abosnjakovic/oversee/releases/download/v0.2.0/oversee-0.2.0-aarch64-apple-darwin.tar.gz"
    sha256 "e96ac62f6ae6f750947d17d07ba8163b26f4b3352e3a3cf098ae7278a2429938"
  else
    url "https://github.com/abosnjakovic/oversee/releases/download/v0.2.0/oversee-0.2.0-x86_64-apple-darwin.tar.gz"
    sha256 "a29572fc87a067fdebc4de92df3a93abdb83746799af78d0ec7c8cc09ca369d5"
  end

  def install
    bin.install "oversee"
  end

  test do
    system "#{bin}/oversee", "--version"
  end
end
