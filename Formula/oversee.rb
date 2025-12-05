class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.2.3"

  if Hardware::CPU.arm?
    url "https://github.com/abosnjakovic/oversee/releases/download/0.2.3/oversee-0.2.3-aarch64-apple-darwin.tar.gz"
    sha256 "3ca1e71bf62f759439f76aff82c1b9ce8f2fde695ea46b8e0e12d6d64b7b482f"
  else
    url "https://github.com/abosnjakovic/oversee/releases/download/0.2.3/oversee-0.2.3-x86_64-apple-darwin.tar.gz"
    sha256 "d96443841cf108776845dd85b4ec661f9e7beb68ed3f26c3cd99a3122fb0561f"
  end

  def install
    bin.install "oversee"
  end

  test do
    system "#{bin}/oversee", "--version"
  end
end
