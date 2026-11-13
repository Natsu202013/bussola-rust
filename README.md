# Bussola

Bussola per **Android e iOS** con il motore di calcolo scritto in **Rust**, UI nativa
per piattaforma. Priorità: **peso minimo**.

Misure reali prese dagli artefatti della release `v1.2.0`:

| | |
|---|---|
| **APK Android** | **83.292 byte** (~81 KB) — era 588 KB prima dell'ottimizzazione del core |
| **Core Rust** | **14.712 byte** (arm64-v8a), **10.508** (armeabi-v7a) — contro 311.312 con `std` |
| **IPA iOS** | **137.223 byte** (~134 KB), eseguibile arm64 Mach-O da **217.944 byte** |
| **Dipendenze** | Rust: solo `libm`. Android: nessuna (nemmeno AndroidX). iOS: SwiftUI + CoreLocation |
| **Permessi Android** | nessuno (il sensore di rotazione non ne richiede) |
| **Target** | Android 7+ (API 24, target 35) · iOS 15+ |

## Perché pesa così poco

1. **`no_std` nel core Rust.** Il 95% dei 311 KB iniziali non era codice: era il
   macchinario dei panic di `std` (`backtrace`, `addr2line`, `gimli`,
   `rustc_demangle`). Togliendo `std` la `.so` scende a 14 KB, cioè ~21x in meno.
   Su Apple serve `-lSystem` esplicito, perché un `cdylib` viene linkato con
   `-nodefaultlibs` e `abort`/`dyld_stub_binder` resterebbero irrisolti.
2. **Niente heap.** Le istanze vivono in un pool statico di 16 slot; handle
   validati su ogni chiamata (NULL, stale, disallineati vengono rifiutati, mai
   dereferenziati).
3. **Android senza AndroidX.** Una `Activity` + una `View` che disegna su
   `Canvas`: nessuna immagine, nessuna libreria grafica, `classes.dex` da 12 KB.
4. **Firma e allineamento corretti.** `.so` allineate a 16 KB (requisito Android
   15), APK firmato, `aligned` a posto.
5. **Icone generate a runtime** oppure, per il launcher, cinque PNG per un totale
   di 35 KB (invece delle ~200 KB di un set adaptive completo).

## Struttura

```
rust/                  core condiviso (no_std, C ABI + JNI)
  src/lib.rs           filtro circolare, normalizzazione, punti cardinali
android/               app Java minimale, disegno su Canvas, zero AndroidX
ios/                   app SwiftUI, progetto generato da XcodeGen
  project.yml          sorgente unica del .xcodeproj
scripts/               build-android-rust.sh, build-ios-rust.sh
.github/workflows/     ci.yml, android.yml, ios.yml
```

Il core espone la stessa matematica a entrambe le piattaforme: filtro passa-basso
**circolare** (interpola lungo l'arco più corto, quindi attraversa il nord a
0°/360° senza fare il giro lungo), normalizzazione degli angoli, conversione da
radianti `atan2` (convenzione Android) e le 16 etichette della rosa dei venti.

## Download

Dalla pagina **Releases** (tag `v1.2.0`). Ogni bump di `versionName` in
`android/app/build.gradle.kts` crea una release nuova al push successivo:

| File | Piattaforma | Note |
|---|---|---|
| `Bussola.apk` | Android | 83 KB, firmato, Android 7+ (API 24), arm64-v8a + armeabi-v7a |
| `Bussola.apk.sha256` | — | checksum |
| `Bussola.ipa` | iOS | 134 KB, **non firmato**, va rifirmato con Sideloadly/AltStore |
| `Bussola.app.zip` | iOS | bundle `.app` per rifirma manuale |
| `Bussola.ipa.sha256` | — | checksum |

## Build

### Core Rust + test (qualsiasi OS)

```bash
cargo test --manifest-path rust/Cargo.toml
```

### Android (locale)

```bash
export ANDROID_NDK_HOME=/percorso/ndk/27.0.12077973
./scripts/build-android-rust.sh     # produce le .so in android/app/src/main/jniLibs
cd android && ./gradlew assembleRelease
# -> android/app/build/outputs/apk/release/app-release.apk
```

La firma release legge quattro variabili d'ambiente: `BUSSOLA_KEYSTORE`,
`BUSSOLA_STORE_PASSWORD`, `BUSSOLA_KEY_ALIAS`, `BUSSOLA_KEY_PASSWORD`.
Senza di esse l'APK esce **non firmato**.

Per includere anche l'emulatore x86_64: `INCLUDE_X86=1 ./scripts/build-android-rust.sh`.

### iOS (serve macOS + Xcode)

```bash
rustup target add aarch64-apple-ios
./scripts/build-ios-rust.sh
brew install xcodegen
cd ios && xcodegen generate
xcodebuild -project Bussola.xcodeproj -scheme Bussola -configuration Release \
  -sdk iphoneos CODE_SIGNING_ALLOWED=NO build
```

Su `INCLUDE_SIM=1 ./scripts/build-ios-rust.sh` produce anche la libreria
universale per il simulatore.

## CI

| Workflow | Runner | Cosa fa |
|---|---|---|
| `ci.yml` | `ubuntu-latest` | 17 test Rust, build, **budget di peso** (64 KB) e verifica allineamento a 16 KB |
| `android.yml` | `ubuntu-latest` | APK firmato + release |
| `ios.yml` | `macos-15` | static lib Rust, XcodeGen, `.app`, IPA non firmato + release |

Tutti e tre allegano i propri file alla release `v<versionName>`; il primo che
arriva la crea, l'altro la completa con i suoi file. Per firmare l'APK con un keystore tuo, aggiungi i secrets
`BUSSOLA_KEYSTORE_BASE64`, `BUSSOLA_STORE_PASSWORD`, `BUSSOLA_KEY_ALIAS`,
`BUSSOLA_KEY_PASSWORD`. Senza, la CI genera un keystore effimero e l'APK è
firmato ma non aggiornabile con build successive.

## Licenza

MIT.
