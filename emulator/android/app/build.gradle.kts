plugins { id("com.android.application") version "8.10.1" }

android {
    namespace = "org.touchhle.android"
    compileSdk = 35
    defaultConfig {
        applicationId = "org.hunterislandpreservation.android"
        minSdk = 26
        targetSdk = 35
        versionCode = 25
        versionName = "0.2.0-preview.12"
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
        release {
            isDebuggable = false
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("localPreview")
        }
    }
    sourceSets {
        getByName("main") {
            java.srcDir("${rootDir.parentFile}/vendor/SDL/android-project/app/src/main/java")
            assets.setSrcDirs(listOf(layout.buildDirectory.dir("generated/preservationAssets")))
            res.setSrcDirs(listOf(layout.buildDirectory.dir("generated/preservationRes")))
            jniLibs.srcDir("jniLibs")
        }
    }
    packaging { jniLibs.useLegacyPackaging = false }
    testOptions {
        unitTests.isIncludeAndroidResources = true
        unitTests.all {
            it.jvmArgs("--add-opens=java.base/java.lang=ALL-UNNAMED",
                "--add-opens=java.base/java.util=ALL-UNNAMED",
                "--add-opens=java.base/java.io=ALL-UNNAMED",
                "--add-opens=java.base/jdk.internal.access=ALL-UNNAMED")
        }
    }
}

dependencies {
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.robolectric:robolectric:4.17")
}

val preparePreservationAssets by tasks.registering(Sync::class) {
    into(layout.buildDirectory.dir("generated/preservationAssets"))
    from("../../touchHLE_fonts") { into("touchHLE_fonts") }
    from("../../touchHLE_dylibs") { into("touchHLE_dylibs") }
    from("../../touchHLE_default_options.txt")
    from("../../../licenses") {
        into("preservation-notices/licenses")
        // These are retained Windows-source references, not Android components.
        exclude("runtime/**", "gnu-runtime/**", "tcl8.6.txt", "tk8.6.txt")
    }
    from("../../../ANDROID-LICENSES.md") { into("preservation-notices"); rename { "LICENSES.md" } }
    from("../../../THIRD_PARTY.md") { into("preservation-notices") }
    from("../../../ANDROID-SOURCE.md") { into("preservation-notices"); rename { "SOURCE.md" } }
}
val preparePreservationRes by tasks.registering(Sync::class) {
    into(layout.buildDirectory.dir("generated/preservationRes"))
    from("src/main/res") { exclude("drawable-nodpi/icon*.png") }
    from("../../res") { include("icon.png", "icon_preview.png", "icon_unofficial.png"); into("drawable-nodpi") }
}
tasks.named("preBuild") { dependsOn(preparePreservationAssets, preparePreservationRes) }
