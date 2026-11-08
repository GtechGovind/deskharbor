plugins {
    id("com.android.application")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

android {
    namespace = "org.deskharbor.deskharbor"
    compileSdk = 36
    ndkVersion = "28.2.13676358"

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        // TODO: Specify your own unique Application ID (https://developer.android.com/studio/build/application-id.html).
        applicationId = "org.deskharbor.deskharbor"
        // You can update the following values to match your application needs.
        // For more information, see: https://flutter.dev/to/review-gradle-config.
        minSdk = 30
        targetSdk = 36
        ndk { abiFilters.add("arm64-v8a") }
        // Uses the version code from pubspec.yaml. When using split APKs, 1000 * ABI_VERSION
        // is added automatically by Flutter. (https://developer.android.com/studio/build/configure-apk-splits#configure-APK-versions)
        // You can force using the value of versionCode by specifying the `-P force-version-code-ignoring-abi=true`
        // flag during build.
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    val keystorePath = System.getenv("DESKHARBOR_KEYSTORE_PATH")
    val keystorePassword = System.getenv("DESKHARBOR_KEYSTORE_PASSWORD")
    val keyAliasValue = System.getenv("DESKHARBOR_KEY_ALIAS")
    val keyPasswordValue = System.getenv("DESKHARBOR_KEY_PASSWORD")
    val releaseRequested = gradle.startParameter.taskNames.any { it.contains("release", ignoreCase = true) }
    if (releaseRequested && listOf(keystorePath, keystorePassword, keyAliasValue, keyPasswordValue).any { it.isNullOrBlank() }) {
        throw GradleException("Release signing requires DESKHARBOR_KEYSTORE_PATH, DESKHARBOR_KEYSTORE_PASSWORD, DESKHARBOR_KEY_ALIAS and DESKHARBOR_KEY_PASSWORD. Debug signing is never used for release.")
    }
    signingConfigs {
        create("deskharborRelease") {
            if (!keystorePath.isNullOrBlank()) {
                storeFile = file(keystorePath)
                storePassword = keystorePassword
                keyAlias = keyAliasValue
                keyPassword = keyPasswordValue
            }
        }
    }
    buildTypes {
        release {
            signingConfig = signingConfigs.getByName("deskharborRelease")
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

flutter {
    source = "../.."
}

tasks.matching { it.name == "preBuild" }.configureEach {
    doFirst {
        if (!file("src/main/jniLibs/arm64-v8a/libdh_ffi.so").isFile) {
            throw GradleException("Build dh-ffi for aarch64-linux-android and copy libdh_ffi.so into app/src/main/jniLibs/arm64-v8a before building Flutter.")
        }
    }
}
