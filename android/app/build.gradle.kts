plugins {
    id("com.android.application")
}

android {
    namespace = "com.bussola.app"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.bussola.app"
        minSdk = 24
        targetSdk = 35
        versionCode = 1
        versionName = "1.0.0"
        // No native build: the .so files are prebuilt by scripts/build-android-rust.sh
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a")
        }
    }

    signingConfigs {
        create("release") {
            // Populated by CI from secrets; absent for local/unsigned builds.
            val storePath = System.getenv("BUSSOLA_KEYSTORE")
            if (storePath != null && file(storePath).exists()) {
                storeFile = file(storePath)
                storePassword = System.getenv("BUSSOLA_STORE_PASSWORD")
                keyAlias = System.getenv("BUSSOLA_KEY_ALIAS") ?: "bussola"
                keyPassword = System.getenv("BUSSOLA_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            // R8 off: the app is a single hand-written class, shrinking it would
            // only risk breaking the JNI names. Proguard config kept for clarity.
            isMinifyEnabled = false
            isShrinkResources = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
            val rel = signingConfigs.getByName("release")
            if (rel.storeFile != null) {
                signingConfig = rel
            }
        }
        debug {
            applicationIdSuffix = ".debug"
            versionNameSuffix = "-debug"
        }
    }

    packaging {
        jniLibs {
            useLegacyPackaging = false
        }
        resources {
            excludes += setOf(
                "META-INF/*.kotlin_module",
                "META-INF/*.version",
                "META-INF/maven/**",
                "kotlin/**",
                "DebugProbesKt.bin"
            )
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    dependenciesInfo {
        includeInApk = false
        includeInBundle = false
    }

    buildFeatures {
        buildConfig = false
        resValues = false
    }

    androidResources {
        // Only the launcher icon and the strings/styles we actually declare.
        noCompress += listOf()
    }

    lint {
        abortOnError = false
        checkReleaseBuilds = false
    }
}
