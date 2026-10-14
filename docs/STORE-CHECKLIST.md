# Free launch store checklist

First launch is free on Google Play and the App Store, and pay-what-you-want on
itch. There is no billing, advertising, paywall, or license-check code.
Store requirements below were checked against official guidance on 2026-10-14.
Bill owns account setup, legal declarations, signing credentials, upload approval,
and publication. No workflow in this repo submits a game automatically.

## Every release

- Choose unique application and save identifiers before players create progress.
- Replace placeholder title, icon, screenshots, support address, and store artwork.
- Test one complete win and lose, replay, mute, persistence, and migration.
- Test touch targets, notches, orientation, suspend/resume, offline use, and low-memory devices.
- Verify data collection declarations against every included library and endpoint.
- Publish a privacy policy and support contact controlled by Bill.
- Keep source and asset licenses with the release; current assets are original MIT material.
- Record version, artifact checksum, device evidence, and release notes.

## itch.io

1. Bill creates the project and selects HTML game.
2. Bill sets zero minimum price and enables optional donations.
3. Bill uploads `dist/game-factory-web.zip` as playable browser content.
4. Bill configures portrait embedding, mobile support, and fullscreen behavior.
5. Bill tests the uploaded build in desktop and mobile browsers.
6. Bill approves publication after checking metadata and privacy links.

The [HTML5 upload guide](https://itch.io/docs/creators/html5) requires `index.html`
at the zip root. The [pricing guide](https://itch.io/docs/creators/pricing)
describes pay-what-you-want. Butler remains disabled without `BUTLER_API_KEY`.
Setting that key is not publication authorization. Do not run the push script
without Bill's explicit instruction.

## Google Play

1. Bill creates and verifies his Play Console account.
2. Bill registers the final package name and chooses a free listing.
3. Bill provides the upload keystore and signing environment variables.
4. Bill enables Play App Signing and secures recovery access.
5. Bill checks the current target SDK and native-library requirements.
6. Bill uploads a signed release AAB to an internal test track.
7. Bill completes content rating, target audience, ads, and Data safety declarations.
8. Bill supplies screenshots, descriptions, privacy policy, and support contact.
9. Bill recruits testers and completes the account's required closed test.
10. Bill requests production access and approves the final rollout.

For personal accounts created after 2023-11-13, the
[official testing policy](https://support.google.com/googleplay/android-developer/answer/14151465)
requires at least twelve testers opted in continuously for fourteen days before
applying for production access. Account status and policy may change; Bill must
check the Console before planning a release date. Debug APK/AAB outputs are for
local verification and cannot substitute for a signed production release.

## App Store

1. Bill enrolls and completes Apple Developer account agreements.
2. Bill registers the final bundle identifier and creates the App Store Connect app.
3. Bill configures his development team, certificates, and provisioning on macOS.
4. Bill validates the Xcode scaffold on simulator and physical devices.
5. Bill archives a signed build and uploads it through Xcode.
6. Bill tests through TestFlight before choosing a free App Store price.
7. Bill completes age rating, export compliance, App Privacy, and store metadata.
8. Bill supplies review notes, privacy policy, support contact, and device screenshots.
9. Bill submits for App Review and chooses the release timing.

Apple documents [privacy declarations](https://developer.apple.com/help/app-store-connect/manage-app-information/manage-app-privacy),
[Xcode distribution](https://help.apple.com/xcode/mac/current/en.lproj/dev067853c94.html),
and [App Review](https://developer.apple.com/app-store/review/).
This Linux host cannot compile Xcode output or provide Bill's signing identity.
