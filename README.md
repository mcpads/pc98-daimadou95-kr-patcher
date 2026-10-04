# 대마도전략물어'95 (PC-98) 한글 패처

Disc Station Vol. 08 Disk 1에 수록된 《대마도전략물어'95》를 꺼내 독립 실행 PC-98 HDM으로 재구성하고 한글 패치를 적용하는 Rust 코드입니다. FAT12와 MZ·LHa 컨테이너 읽기, MADDAT(FLINK)·GCS·Compile LZ 형식 처리, 번역 검증과 글리프·조판 감사, 한글 폰트 뱅크와 그래픽 합성, typed V30 훅과 Expected Write 검사를 제공합니다.

배포용 패치와 적용 방법은 [마도물어 시리즈 한글 번역 프로젝트](https://github.com/mcpads/madou-monogatari-kr-patch/tree/main/pc98-daimadou-senryaku-95)에서 제공합니다.

## 제공하지 않는 것

이 저장소에는 원본 디스크, 빌더가 만든 HDM, 번역 JSON, 폰트 파일, 아르르 그래픽과 타이틀 그래픽 원화가 없습니다. 따라서 이 저장소만으로는 배포 패치를 다시 만들 수 없습니다. 아래 입력을 직접 갖춘 경우에만 한글판 HDM을 생성합니다.

## 빌드와 테스트

```bash
cargo build --release
cargo test
```

의존성 일부(`v30`, `expected-write`)는 [retro-typed-isa](https://github.com/mcpads/retro-typed-isa) 저장소에서 받습니다.

기본 테스트는 합성 입력만 사용합니다. 원본 디스크, 폰트, 그래픽이나 번역이 필요한 테스트는 `#[ignore = "requires ..."]`로 필요한 입력을 밝혀 두었습니다. 입력을 갖춘 뒤 `cargo test -- --ignored`로 실행하며, 입력이 없으면 성공으로 넘어가지 않고 실패합니다.

| 테스트 입력 | 지정 방법 |
| --- | --- |
| 원본 Disk 1 HDM | 환경 변수 `DS8_DISK1` |
| 디스크에서 꺼낸 `MADDAT` | `ENDING_CREDITS_MADDAT`, `STAGE_SELECT_MADDAT`, `TITLE_COMPACT_MADDAT`, `TITLE_TRANSLATION_MADDAT` |
| 디스크에서 꺼낸 게임 파일 디렉터리 | `GRAPHIC_TEXT_PAYLOAD_DIR`, `STATE_GRAPHIC_PAYLOAD_DIR`, `ENDING_CREDITS_PAYLOAD_DIR` |
| 미리보기 출력 경로 | `ENDING_CREDITS_PREVIEW_PATH`, `TITLE_TRANSLATION_PREVIEW_PATH` |
| 번역·폰트·그래픽 | 아래 빌드 입력과 같은 경로 |

## 지원 원본

Disc Station Vol. 08 Disk 1의 헤더 없는 HDM을 지원합니다. 빌더는 크기와 SHA-256이 다르면 진행하지 않습니다.

| 항목 | 값 |
| --- | --- |
| 크기 | 1,261,568바이트 |
| SHA-256 | `b2ade325198210914e1cd1a6add3724fb4de1a1c5e5f8f617acfe7999234c122` |

## 빌드 입력

| 입력 | 경로 | 비고 |
| --- | --- | --- |
| 번역 | `assets/translations/index.json`, `assets/translations/segments/*.json` | `--drafts`로 지정 |
| 폰트 | `assets/fonts/NeoDunggeunmo.ttf`, `assets/fonts/NeoDunggeunmo-OFL.txt` | [Neo둥근모](https://github.com/neodgm/neodgm) 1.600 |
| 타이틀 그래픽 | `assets/title/daimadou-title-logo.imagegen.png`, `assets/title/daimadou-title-ribbon.imagegen.png` | |
| 아르르 그래픽 | `assets/characters/arle/` | 커스텀 아르르판에만 필요 |

폰트 프로필(`assets/fonts/*.json`)은 저장소에 있습니다. 라이선스 문서에는 `SIL OPEN FONT LICENSE` 문구가 있어야 합니다.

폰트와 그래픽은 아래 SHA-256과 같아야 합니다. 다르면 해당 단계에서 빌드를 멈춥니다.

```text
d61b60eccb731f8ca9c7da582e4a05a94db66b570471809950aa9a7261b941d6  assets/fonts/NeoDunggeunmo.ttf
b0dc12c96a15184dab7ebcb788918e0c80486f47f1564d6fc093ad3153c79722  assets/title/daimadou-title-logo.imagegen.png
26d49834c571e5f3d23229711bdcc2cbb048dee7c8195f748103e303dcb8ae16  assets/title/daimadou-title-ribbon.imagegen.png
efdd0537c914a1c31a52c3e879c8b7c89efd1d3b6ecac0aceb2e29186723ecc4  assets/characters/arle/large-portrait.pc98.png
ffd941b0d5bb7f7d3ddd475b2ff670473f6544591984c02126aa5d23293c8e04  assets/characters/arle/opening-poses.pc98.png
a6644d7868d6ad60488db7f6bbf62381d99f73dfb039398d5b80cf929b5d20b0  assets/characters/arle/small-status.pc98.png
abc6f393708efa54511b7a8cb1404979d576fce03c31f8e813475293be347cc7  assets/characters/arle/battle-sprites.pc98.png
ab7dc91b5f0a8f3fc97ddc7171468ac82b237562fff52d9bf29170449888760e  assets/characters/arle/ending-meal.pc98.png
```

`--arle-assets <디렉터리>`를 주면 같은 다섯 파일 이름을 가진 다른 아르르 그래픽 세트를 쓸 수 있습니다. 이때는 해시 대신 크기·팔레트·타일 용량을 검사합니다.

## 한글판 HDM 생성

```bash
# 커스텀 아르르
cargo run --release -- build-full-translation \
  --source <Disk1.hdm> --drafts assets/translations --output out/custom-arle.hdm

# 원본 아르르
cargo run --release -- build-full-translation \
  --source <Disk1.hdm> --drafts assets/translations --preserve-original-arle \
  --output out/original-arle.hdm
```

빌더는 원본 디스크를 바꾸지 않고 기존 출력 파일을 덮어쓰지 않습니다. 결과는 Disc Station 셸을 거치지 않는 독립 실행 HDM이며, 바뀌는 게임 파일은 `MADDAT`, `MAD.COM`, `SELECT.COM`, `OPENING.COM`, `ENDING.COM` 다섯 개입니다.

## 배포 패치 생성

배포 ZIP은 [Retro Patcher](https://github.com/mcpads/retro-patcher)의 `retro-patch-author`로 만듭니다. `distribution/`의 계획 파일은 원본 파일 정체와 파일별 변환 방식을 선언합니다.

```bash
retro-patch-author create distribution/custom-arle.plan.json <Disk1.hdm> out/custom-arle.hdm custom-arle.zip
retro-patch-author create distribution/original-arle.plan.json <Disk1.hdm> out/original-arle.hdm original-arle.zip
```

배포 패치 v1.0.0의 두 ZIP은 위 입력과 명령으로 만든 결과와 바이트 단위로 같습니다.

## 그 밖의 명령

`verify-source`, `survey-source`, `build`(일본어판 독립 실행 디스크), 번역 작업 공간·검증·감사 명령과 단계별 개발 빌드의 사용법은 `cargo run -- help <명령>`으로 확인할 수 있습니다.

## 라이선스

이 저장소의 소스 코드는 [MIT License](LICENSE)로 제공합니다.
