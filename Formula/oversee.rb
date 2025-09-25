class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.1.4"

  # Apple Silicon only
  depends_on arch: :arm64
  url "https://github.com/abosnjakovic/oversee/releases/download/v0.1.4/oversee-0.1.4-aarch64-apple-darwin.tar.gz"
  sha256 "87973911c24670794a79b80f3780490034d459b3dbcdd32a7548474ba2d9319a"

  def install
    bin.install "oversee"
  end

  test do
    system "#{bin}/oversee", "--version"
  end
end
