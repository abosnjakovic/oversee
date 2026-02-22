class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.2.8"

  if Hardware::CPU.arm?
    url "https://github.com/abosnjakovic/oversee/releases/download/v0.2.8/oversee-0.2.8-aarch64-apple-darwin.tar.gz"
    sha256 "eb5e0cc670e123604b2dd2c3996be4df9d82ca8cf8f0ad4dd8932be914db2ddf"
  else
    url "https://github.com/abosnjakovic/oversee/releases/download/v0.2.8/oversee-0.2.8-x86_64-apple-darwin.tar.gz"
    sha256 "a95cf90ce7516124a568274e743487b08365b0b63b0d9818acd361d9cce1a3b4"
  end

  def install
    bin.install "oversee"
  end

  test do
    system "#{bin}/oversee", "--version"
  end
end
