class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.1.9"

  if Hardware::CPU.arm?
    url "https://github.com/abosnjakovic/oversee/releases/download/v0.1.9/oversee-0.1.9-aarch64-apple-darwin.tar.gz"
    sha256 "ccc3ba2d7b1d9552e1e65fccf1deb4e09d67c4dbb141bda99c56f0e53bf4f2b3"
  else
    url "https://github.com/abosnjakovic/oversee/releases/download/v0.1.9/oversee-0.1.9-x86_64-apple-darwin.tar.gz"
    sha256 "0a4c2dd554549777ec1df8de1930370f6eeda00d3bc9683b9fcb929b095bd702"
  end

  def install
    bin.install "oversee"
  end

  test do
    system "#{bin}/oversee", "--version"
  end
end
