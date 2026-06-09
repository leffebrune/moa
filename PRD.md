# PRD: Moa

## 1. 개요

**Moa**는 개인 지식 데이터베이스로 사용할 로컬 기반 마크다운 메모 앱이다.

이름의 의미는 “메모와 지식을 모아두는 앱”이다. 영어 표기는 **Moa**로 사용한다.

현재 목적은 순수 개인 사용이다. 공개 배포, 사용자 확장, 협업, 클라우드 서비스화는 고려하지 않는다.

## 2. 목표

* 개인 메모와 지식 문서를 빠르게 작성한다.
* 문서를 마크다운 파일로 저장한다.
* 보기 모드에서는 마크다운을 렌더링해서 읽는다.
* 편집 모드에서는 마크다운 원본 소스를 plain text로 직접 편집한다.
* 문서를 카테고리와 태그로 간단히 정리한다.
* FTS 검색과 벡터 검색을 제공한다.
* 모든 원본 데이터는 로컬에 저장한다.
* SQLite는 원본 저장소가 아니라 검색과 메타데이터 관리를 위한 보조 저장소로 사용한다.

## 3. 범위

### 포함

* 문서 생성
* 문서 보기
* 문서 편집
* 문서 삭제
* 마크다운 렌더링
* 마크다운 원본 편집
* 카테고리 지정
* 태그 지정
* 로컬 파일 저장
* SQLite 메타데이터 저장
* FTS 검색
* 벡터 검색
* 인덱스 재생성
* 간단한 설정

### 제외

* 공개 배포
* 계정
* 로그인
* 클라우드 동기화
* 협업 편집
* 공유 링크
* 플러그인
* 블로그 발행
* 모바일 우선 최적화
* WYSIWYG 편집기
* Notion식 블록 에디터
* 복잡한 LLM 워크플로
* 추후 기능 확장을 전제로 한 설계

## 4. 저장 전략

Moa는 **마크다운 파일 원본 + SQLite 보조 저장소** 구조를 사용한다.

```txt
.md 파일 = 원본 데이터
SQLite = 메타데이터, 검색 인덱스, 벡터 인덱스용 보조 저장소
```

SQLite에 본문 전체를 원본으로 저장하지 않는다.

본문은 `.md` 파일이 기준이며, SQLite에 저장되는 본문, 청크, 임베딩은 모두 검색을 위한 파생 데이터로 본다.

SQLite 파일이 삭제되어도 `.md` 파일만 있으면 앱이 다시 인덱스를 재생성할 수 있어야 한다.

## 5. 파일 구조

```txt
moa-vault/
  notes/
    2026-06-08-local-first-note-app.md
    2026-06-08-vector-search.md

  attachments/

  .moa/
    moa.sqlite
    settings.json
```

## 6. 마크다운 문서 포맷

각 문서는 YAML frontmatter를 가진다.

```markdown
---
id: 9f2a7c4e-42f2-4c80-b921-123456789abc
title: 로컬 메모앱 설계
category: project
tags:
  - markdown
  - local
  - search
created_at: 2026-06-08T10:30:00+09:00
updated_at: 2026-06-08T11:10:00+09:00
---

# 로컬 메모앱 설계

본문 내용...
```

## 7. 데이터 원칙

* `.md` 파일이 원본이다.
* SQLite는 언제든 재생성 가능해야 한다.
* 문서 ID는 파일명이 아니라 frontmatter의 `id`를 기준으로 한다.
* 카테고리와 태그는 frontmatter에 저장한다.
* SQLite에는 카테고리와 태그를 캐싱한다.
* FTS 인덱스는 `.md` 파일에서 재생성 가능해야 한다.
* 벡터 인덱스도 `.md` 파일에서 재생성 가능해야 한다.
* 파일명은 사람이 읽기 쉬운 형태로 만든다.
* 파일명이 바뀌어도 frontmatter의 `id`가 같으면 같은 문서로 본다.

## 8. 핵심 기능

## 8.1 문서 보기 및 편집

문서는 **보기 모드**와 **편집 모드**를 분리한다.

WYSIWYG 편집기는 사용하지 않는다.

```txt
보기 모드: Markdown → HTML 렌더링
편집 모드: Markdown source plain text 편집
```

### 보기 모드

* 문서를 처음 열면 보기 모드로 표시한다.
* `.md` 본문을 마크다운 렌더러로 렌더링한다.
* frontmatter는 화면에 직접 노출하지 않는다.
* 제목, 카테고리, 태그는 별도 UI로 표시한다.
* 실수로 문서가 수정되지 않도록 기본 상태는 보기 모드로 둔다.

### 편집 모드

* 사용자가 명시적으로 편집 모드에 진입한다.
* 편집 모드에서는 마크다운 원본 본문을 plain text로 표시한다.
* 사용자는 마크다운 문법을 직접 입력하고 수정한다.
* frontmatter는 편집기에 직접 노출하지 않는다.
* 제목, 카테고리, 태그는 별도 입력 UI로 수정한다.
* WYSIWYG 변환이나 블록 기반 편집은 제공하지 않는다.

### 저장 정책

* 문서 본문은 편집 중 자동 저장한다.
* 자동 저장은 입력마다 즉시 실행하지 않고 debounce를 둔다.
* 문서 전환, 앱 종료, 편집 모드 종료 시에는 저장을 강제로 flush한다.
* 저장 시 frontmatter와 body를 조합해 `.md` 파일에 기록한다.
* 저장 후 SQLite 메타데이터와 FTS 인덱스를 갱신한다.
* 벡터 인덱스는 즉시 갱신하지 않고 재생성 대기열에 추가할 수 있다.

### 편집기 구현

* 초기 구현은 plain text 기반 마크다운 편집으로 한다.
* 권장 에디터는 CodeMirror 6이다.
* MVP에서는 더 단순하게 `<textarea>`로 시작할 수도 있다.
* split view는 MVP에 포함하지 않는다.

## 8.2 카테고리

* 문서는 하나의 카테고리를 가질 수 있다.
* 카테고리별 문서 목록을 볼 수 있다.
* 카테고리는 frontmatter와 SQLite 양쪽에 반영된다.
* 카테고리를 제거하면 문서는 미분류 상태가 된다.

## 8.3 태그

* 문서는 여러 개의 태그를 가질 수 있다.
* 태그별 문서 목록을 볼 수 있다.
* 태그 입력 시 기존 태그 자동완성을 제공한다.
* 태그는 frontmatter와 SQLite 양쪽에 반영된다.

## 8.4 FTS 검색

검색 대상은 다음과 같다.

* 제목
* 본문
* 카테고리
* 태그

기능은 다음과 같다.

* 키워드 검색
* 결과 스니펫 표시
* 관련도 정렬
* 수정일 정렬
* 카테고리 필터
* 태그 필터

## 8.5 벡터 검색

* 문서를 청크 단위로 나눈다.
* 각 청크의 임베딩을 생성한다.
* 검색어 임베딩과 문서 청크 임베딩을 비교한다.
* 의미적으로 가까운 문서를 결과로 보여준다.
* 벡터 인덱스는 로컬에 저장한다.
* 벡터 인덱스는 삭제되어도 다시 생성 가능해야 한다.

## 9. SQLite 저장 항목

SQLite에는 다음 데이터를 저장한다.

```txt
documents
- id
- path
- title
- category
- created_at
- updated_at
- file_mtime
- content_hash
- last_indexed_at

tags
- id
- name

document_tags
- document_id
- tag_id

fts_index
- document_id
- title
- body
- category
- tags

embeddings
- id
- document_id
- chunk_index
- chunk_hash
- embedding
- model_name
- created_at
```

## 10. 인덱싱 흐름

앱 시작 시:

```txt
vault 폴더 스캔
→ .md 파일 확인
→ frontmatter 파싱
→ 변경된 파일 감지
→ SQLite 메타데이터 갱신
→ FTS 인덱스 갱신
→ 필요한 경우 벡터 인덱스 갱신
```

문서 저장 시:

```txt
.md 파일 저장
→ SQLite 메타데이터 갱신
→ FTS 인덱스 갱신
→ 벡터 인덱스 갱신 대기열에 추가
```

SQLite 재생성 시:

```txt
.moa/moa.sqlite 삭제 또는 손상
→ notes 폴더 전체 스캔
→ frontmatter 파싱
→ documents 재생성
→ tags 재생성
→ FTS 재생성
→ embeddings는 필요 시 재생성
```

## 11. 기술 스택

```txt
App Framework: Tauri V2
Backend: Rust
Frontend: Svelte + TypeScript + Vite
Editor: CodeMirror 6
Markdown Renderer: markdown-it 또는 marked
Local DB: SQLite
FTS: SQLite FTS5
Vector Search: SQLite 기반 또는 Rust 기반 로컬 인덱스
Document Format: Markdown + YAML frontmatter
```

## 12. 역할 분리

### Svelte 프론트엔드

* 문서 목록 표시
* 문서 보기 모드
* 문서 편집 모드
* 검색창
* 카테고리/태그 UI
* 설정 화면
* Rust command 호출

### Rust 백엔드

* `.md` 파일 읽기
* `.md` 파일 쓰기
* frontmatter 파싱
* SQLite 관리
* FTS 인덱싱
* 벡터 인덱싱
* 검색 실행
* vault 스캔
* 파일 변경 감지

## 13. 화면 구성

## 13.1 메인 화면

```txt
좌측: 카테고리 / 태그 / 전체 문서
중앙: 문서 목록 또는 검색 결과
우측: 문서 보기 또는 편집 영역
```

## 13.2 문서 화면

* 보기 모드
* 편집 모드
* 제목 입력
* 카테고리 선택
* 태그 입력
* 저장 상태 표시

## 13.3 검색 화면

* 검색 입력창
* FTS 검색
* 벡터 검색
* 카테고리 필터
* 태그 필터
* 검색 결과 목록

## 13.4 설정 화면

* vault 위치
* 인덱스 재생성
* 벡터 검색 사용 여부
* 임베딩 모델 설정
* 데이터 상태 확인

## 14. MVP 구현 순서

### 1단계: 기본 앱 구조

* Tauri V2 프로젝트 생성
* Svelte + TypeScript + Vite 설정
* Rust command 연결
* vault 폴더 설정

### 2단계: 문서 파일 관리

* `.md` 파일 생성
* frontmatter 생성
* 문서 읽기
* 문서 저장
* 문서 삭제
* 문서 목록 표시

### 3단계: 보기/편집 모드

* 보기 모드 마크다운 렌더링
* 편집 모드 plain text 편집
* 자동 저장
* 편집 모드 종료 시 저장 flush
* 제목/카테고리/태그 UI 분리

### 4단계: SQLite 메타데이터

* documents 테이블
* tags 테이블
* document_tags 테이블
* vault 스캔
* 변경 감지
* SQLite 재생성

### 5단계: FTS 검색

* FTS 테이블 생성
* 본문 인덱싱
* 제목/본문 검색
* 태그/카테고리 필터
* 검색 결과 스니펫

### 6단계: 벡터 검색

* 문서 청킹
* 임베딩 생성
* 벡터 저장
* 의미 검색
* FTS 검색과 결과 통합

## 15. 성공 기준

* 앱 이름은 Moa로 사용한다.
* 앱을 열고 바로 문서를 볼 수 있다.
* 명시적으로 편집 모드에 들어가 마크다운 원본을 수정할 수 있다.
* 문서는 `.md` 파일로 직접 확인할 수 있다.
* SQLite를 삭제해도 `.md` 파일에서 인덱스를 복구할 수 있다.
* 카테고리와 태그로 문서를 정리할 수 있다.
* FTS 검색으로 원하는 문서를 찾을 수 있다.
* 벡터 검색으로 표현이 다른 관련 문서를 찾을 수 있다.
* 인터넷 없이 기본 기능이 동작한다.
