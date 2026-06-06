# Homebrew Cask for Sentynyx.
#
# This is the template that lives in the homebrew tap repo
# (edenadiv/homebrew-tap). The release workflow updates `version` and the
# `sha256` of the published universal .dmg on each tagged release.
#
#   brew install --cask edenadiv/tap/sentynyx
#
cask "sentynyx" do
  version "0.4.0"
  sha256 "REPLACE_WITH_DMG_SHA256_AT_RELEASE"

  url "https://github.com/edenadiv/sentynyx-app/releases/download/v#{version}/Sentynyx_#{version}_universal.dmg",
      verified: "github.com/edenadiv/sentynyx-app/"
  name "Sentynyx"
  desc "Local-first privacy perimeter for using any LLM without leaking data"
  homepage "https://github.com/edenadiv/sentynyx-app"

  depends_on macos: ">= :big_sur"

  app "Sentynyx.app"

  zap trash: [
    "~/Library/Application Support/Sentynyx",
    "~/Library/Caches/com.sentynyx.app",
    "~/Library/Preferences/com.sentynyx.app.plist",
  ]
end
