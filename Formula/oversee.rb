class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "v0.1.8"

  if Hardware::CPU.arm?
    url "https://github.com/abosnjakovic/oversee/releases/download/vv0.1.8/oversee-v0.1.8-aarch64-apple-darwin.tar.gz"
    sha256 "6078357910102744e8249a76dfa9676eaef73c6ad6029f5d479d9696146108b4"
  else
    url "https://github.com/abosnjakovic/oversee/releases/download/vv0.1.8/oversee-v0.1.8-x86_64-apple-darwin.tar.gz"
    sha256 "90af07ccd18988d3490085e49f56345dd13df25ce312dbe06daf63c5a44ae2a8"
  end

  def install
    bin.install "oversee"
  end

  test do
    system "#{bin}/oversee", "--version"
  end
end
