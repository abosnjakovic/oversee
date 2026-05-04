class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.3.0"

  if Hardware::CPU.arm?
    url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.0/oversee-0.3.0-aarch64-apple-darwin.tar.gz"
    sha256 "7846c390ee4b7464b8a060b31e7ad300e67794aaf5e55c49920f8e9d78c3e415"
  else
    url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.0/oversee-0.3.0-x86_64-apple-darwin.tar.gz"
    sha256 "950bd5fca7dbebdab449b85027ad002e7825ca80a7eb4a2e1a09f11261b3b0de"
  end

  def install
    bin.install "oversee"
  end

  test do
    system "#{bin}/oversee", "--version"
  end
end
