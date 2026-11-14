# SPDX-License-Identifier: MPL-2.0
from pathlib import Path
root=Path(__file__).resolve().parents[1];android=root/'emulator/android'
if 'org.hunterislandpreservation.android' in (android/'app/build.gradle.kts').read_text():
 print('Android preview is already configured; preserve subsequent changes and tests.')
 raise SystemExit(0)
(android/'settings.gradle.kts').write_text('''pluginManagement { repositories { google(); mavenCentral(); gradlePluginPortal() } }
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories { google(); mavenCentral() }
}
rootProject.name = "HunterIslandPreservation"
include(":app")
''')
(android/'app/build.gradle.kts').write_text('''plugins { id("com.android.application") version "8.10.1" }

android {
    namespace = "org.touchhle.android"
    compileSdk = 35
    defaultConfig {
        applicationId = "org.hunterislandpreservation.android"
        minSdk = 26
        targetSdk = 35
        versionCode = 9
        versionName = "0.1.0-android9"
        ndk { abiFilters += "arm64-v8a" }
    }
    buildFeatures { buildConfig = true }
    signingConfigs {
        create("localPreview") {
            storeFile = file(System.getenv("HI_ANDROID_SIGNING_FILE") ?: error("Run the project signing preparation first"))
            storePassword = System.getenv("HI_ANDROID_SIGNING_PASSWORD")
            keyAlias = "hunter-preview"
            keyPassword = System.getenv("HI_ANDROID_SIGNING_PASSWORD")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    buildTypes {
        debug { isDebuggable = true; signingConfig = signingConfigs.getByName("localPreview") }
        release { isMinifyEnabled = false }
    }
    sourceSets {
        getByName("main") {
            java.srcDir("${rootDir.parentFile}/vendor/SDL/android-project/app/src/main/java")
            assets.setSrcDirs(listOf(layout.buildDirectory.dir("generated/preservationAssets")))
            jniLibs.srcDir("jniLibs")
        }
    }
    packaging { jniLibs.useLegacyPackaging = false }
}

val preparePreservationAssets by tasks.registering(Sync::class) {
    into(layout.buildDirectory.dir("generated/preservationAssets"))
    from("../../touchHLE_fonts") { into("touchHLE_fonts") }
    from("../../touchHLE_dylibs") { into("touchHLE_dylibs") }
    from("../../touchHLE_default_options.txt")
    from("../../../licenses") { into("preservation-notices/licenses") }
    from("../../../ANDROID-LICENSES.md") { into("preservation-notices"); rename { "LICENSES.md" } }
}
tasks.named("preBuild") { dependsOn(preparePreservationAssets) }
''')
(android/'app/src/main/AndroidManifest.xml').write_text('''<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    <uses-feature android:name="android.hardware.touchscreen" android:required="false" />
    <uses-feature android:glEsVersion="0x00030000" android:required="true" />
    <!-- Local offline preservation: no INTERNET or shared-storage permission. -->
    <application android:allowBackup="false" android:fullBackupContent="false"
        android:hardwareAccelerated="true" android:icon="@drawable/icon_unofficial"
        android:label="Hunter Island · touchHLE" android:theme="@android:style/Theme.Material.Light.NoActionBar">
        <activity android:name=".LauncherActivity" android:exported="true"
            android:configChanges="orientation|screenSize|keyboardHidden">
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
        <activity android:name=".MainActivity" android:exported="false"
            android:configChanges="layoutDirection|locale|orientation|uiMode|screenLayout|screenSize|smallestScreenSize|keyboard|keyboardHidden|navigation"
            android:screenOrientation="landscape" android:launchMode="singleTask"
            android:theme="@android:style/Theme.NoTitleBar.Fullscreen" />
    </application>
</manifest>
''')
(android/'gradle.properties').write_text('''org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8
org.gradle.parallel=false
android.useAndroidX=false
android.nonTransitiveRClass=true
''')
print('Separate native Android launcher and reproducible asset staging configured.')
