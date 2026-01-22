class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "v0.2.5"

  if Hardware::CPU.arm?
    url "https://github.com/abosnjakovic/oversee/releases/download/vv0.2.5/oversee-v0.2.5-aarch64-apple-darwin.tar.gz"
    sha256 "b2e98f6705b074d2efb4640346792f9f0a2cd40bd599a52b88ef382a4bf3473e"
  else
    url "https://github.com/abosnjakovic/oversee/releases/download/vv0.2.5/oversee-v0.2.5-x86_64-apple-darwin.tar.gz"
    sha256 "e97acadd1ebd4ad029b401cf16d73a2ffe8774ffbfabd3b83bf2924638943ff0"
  end

  def install
    bin.install "oversee"
  end

  test do
    system "#{bin}/oversee", "--version"
  end
end
