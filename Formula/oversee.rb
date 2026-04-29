class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.2.9"

  if Hardware::CPU.arm?
    url "https://github.com/abosnjakovic/oversee/releases/download/v0.2.9/oversee-0.2.9-aarch64-apple-darwin.tar.gz"
    sha256 "f3c9d0f73f1318c8797340ae74d6909fe3b8ffa9e14fcabd93393e3d07061860"
  else
    url "https://github.com/abosnjakovic/oversee/releases/download/v0.2.9/oversee-0.2.9-x86_64-apple-darwin.tar.gz"
    sha256 "303b76d026f877722b24bbbb36aa6ace4b77998fc3c0413499285c3d24e3d26a"
  end

  def install
    bin.install "oversee"
  end

  test do
    system "#{bin}/oversee", "--version"
  end
end
