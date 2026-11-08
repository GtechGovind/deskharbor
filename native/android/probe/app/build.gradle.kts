plugins { id("com.android.application"); id("org.jetbrains.kotlin.android") }
android {
    namespace = "org.deskharbor.probe"
    compileSdk = 35
    defaultConfig { applicationId = "org.deskharbor.probe"; minSdk = 30; targetSdk = 35; versionCode = 1; versionName = "0.0.1-p0" }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
    kotlinOptions { jvmTarget = "17" }
}
dependencies { testImplementation("junit:junit:4.13.2") }
