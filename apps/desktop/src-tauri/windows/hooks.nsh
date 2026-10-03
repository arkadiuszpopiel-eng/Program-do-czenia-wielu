; Haki instalatora NSIS Alfy (tauri.bundle.conf.json → bundle > windows > nsis > installerHooks).
; Plik w UTF-8 z BOM (NSIS bez BOM czyta !include w stronie kodowej ANSI — polskie napisy).
; Instalacja per-user bez UAC do $LOCALAPPDATA\Alfa (ADR 0007, PLAN §1.2):
;   alfa.exe            — stały launcher (skrót w Menu Start z AUMID, protokół alfa://, „Otwórz w Alfie”)
;   versions\<ver>\     — wersje obok siebie (alfa-desktop.exe, version.json, notes.md)
;   current.json        — wersja aktywna i poprzednia (zapis atomowy: alfa.exe --alfa-installed <ver>)
;   webview-data\       — stały folder danych WebView2 poza katalogami wersji
; Tauri kopiuje aplikację jako $INSTDIR\alfa.exe (mainBinaryName) i launcher jako alfa-launcher.exe
; (externalBin); POSTINSTALL przenosi aplikację do versions\${VERSION}\ i stawia launcher na stałej
; ścieżce. Bez alfa-launcher.exe (build bez nakładki) instalacja zostaje płaska (bez aktualizacji).
; Sidecary (llama-server, whisper-server, piper) NIE są w instalatorze — trafiają osobno (przy pierwszym
; użyciu / w Ustawieniach) do $LOCALAPPDATA\Alfa\sidecars\<silnik>\ i przeżywają aktualizacje (docs/RELEASE.md).

; Działająca Alfa w dowolnej wersji — zamknięcie przez Restart Manager (pytanie jak przy launcherze).
!macro ALFA_CLOSE_RUNNING_VERSIONS
  ClearErrors
  FindFirst $R8 $R9 "$INSTDIR\versions\*.*"
  ${DoWhile} $R9 != ""
    ${If} ${FileExists} "$INSTDIR\versions\$R9\alfa-desktop.exe"
      !insertmacro CheckIfAppIsRunning "$INSTDIR\versions\$R9\alfa-desktop.exe" "${PRODUCTNAME}"
    ${EndIf}
    FindNext $R8 $R9
  ${Loop}
  FindClose $R8
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro ALFA_CLOSE_RUNNING_VERSIONS
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ${If} ${FileExists} "$INSTDIR\alfa-launcher.exe"
    ; Ponowna instalacja tej samej wersji zastępuje jej katalog (działające wersje zamknięto wyżej).
    RMDir /r "$INSTDIR\versions\${VERSION}"
    CreateDirectory "$INSTDIR\versions\${VERSION}"
    Rename "$INSTDIR\${MAINBINARYNAME}.exe" "$INSTDIR\versions\${VERSION}\alfa-desktop.exe"
    Rename "$INSTDIR\alfa-launcher.exe" "$INSTDIR\${MAINBINARYNAME}.exe"
    ; Launcher z instalatora jest aktualny — przygotowana wcześniej zamiana jest nieaktualna.
    Delete "$INSTDIR\alfa.exe.new"
    Delete "$INSTDIR\alfa.exe.new.sha256"
    CreateDirectory "$INSTDIR\webview-data"
    ; version.json, current.json (nowa wersja czeka na zdrowy start; poprzednia zostaje do
    ; przywrócenia) i sprzątanie (zostają 2 wersje) — w Rust, atomowo.
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --alfa-installed ${VERSION}' $0
    ${If} $0 <> 0
      DetailPrint "Launcher nie przełączył wersji (kod $0) — szczegóły w launcher.log."
    ${EndIf}
  ${EndIf}
  ; „Otwórz w Alfie” dla plików i folderów (HKCU, bez UAC) — ścieżka trafia do Alfy przez launcher.
  WriteRegStr HKCU "Software\Classes\*\shell\Alfa" "" "Otwórz w Alfie"
  WriteRegStr HKCU "Software\Classes\*\shell\Alfa" "Icon" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\",0"
  WriteRegStr HKCU "Software\Classes\*\shell\Alfa\command" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
  WriteRegStr HKCU "Software\Classes\Directory\shell\Alfa" "" "Otwórz w Alfie"
  WriteRegStr HKCU "Software\Classes\Directory\shell\Alfa" "Icon" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\",0"
  WriteRegStr HKCU "Software\Classes\Directory\shell\Alfa\command" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro ALFA_CLOSE_RUNNING_VERSIONS
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    ; Pliki programu: wersje, stan launchera i aktualizacji, „Otwórz w Alfie”.
    RMDir /r "$INSTDIR\versions"
    RMDir /r "$INSTDIR\staging"
    Delete "$INSTDIR\current.json"
    Delete "$INSTDIR\updates.json"
    Delete "$INSTDIR\launcher.log"
    Delete "$INSTDIR\running.lock"
    Delete "$INSTDIR\alfa.exe.new"
    Delete "$INSTDIR\alfa.exe.new.sha256"
    Delete "$INSTDIR\alfa.exe.old"
    DeleteRegKey HKCU "Software\Classes\*\shell\Alfa"
    DeleteRegKey HKCU "Software\Classes\Directory\shell\Alfa"
    ; Dane (rozmowy, pamięć, modele, sidecary, logi, dane WebView2, konfiguracja) — tylko gdy
    ; zaznaczono „Usuń także dane Alfy”; domyślnie zostają. Pliki w %USERPROFILE%\Alfa (katalogi
    ; robocze sesji) i klucze w Menedżerze poświadczeń Windows nie są usuwane nigdy.
    ${If} $DeleteAppDataCheckboxState = 1
      SetShellVarContext current
      RMDir /r "$LOCALAPPDATA\Alfa"
      RMDir /r "$APPDATA\Alfa"
    ${EndIf}
    RMDir "$INSTDIR"
  ${EndIf}
!macroend
