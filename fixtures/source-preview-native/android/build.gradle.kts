buildscript {
    repositories {
        google()
        mavenCentral()
    }
    dependencies {
        classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:2.4.21")
    }
}

plugins {
    id("com.android.application") version "9.5.2"
    id("org.jetbrains.kotlin.plugin.compose") version "2.4.21"
}

android {
    namespace = "sh.dui.sourcepreview"
    compileSdk {
        version = release(37) {
            minorApiLevel = 1
        }
    }
    defaultConfig {
        applicationId = "sh.dui.sourcepreview"
        minSdk = 26
        targetSdk = 35
    }
    buildFeatures { compose = true }
    // Compile fixtures/source-preview/Settings.kt unchanged; the .swift file there is ignored.
    sourceSets.getByName("main").kotlin.srcDir("../../source-preview")
}

dependencies {
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation(platform("androidx.compose:compose-bom:2026.11.01"))
    implementation("androidx.compose.material3:material3")
}
