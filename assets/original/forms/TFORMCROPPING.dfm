object FormCropping: TFormCropping
  Left = 618
  Top = 293
  Width = 600
  Height = 429
  HorzScrollBar.Visible = False
  VertScrollBar.Visible = False
  BorderWidth = 10
  Caption = 'Cropping'
  Color = clBtnFace
  Font.Charset = DEFAULT_CHARSET
  Font.Color = clWindowText
  Font.Height = -11
  Font.Name = 'MS Sans Serif'
  Font.Style = []
  OldCreateOrder = False
  OnClose = FormClose
  OnCreate = FormCreate
  OnResize = FormResize
  OnShow = FormShow
  PixelsPerInch = 96
  TextHeight = 13
  object PanelBottom: TPanel
    Left = 0
    Top = 179
    Width = 564
    Height = 192
    Align = alBottom
    AutoSize = True
    BevelOuter = bvNone
    TabOrder = 1
    object SpeedButtonSelectFile: TSpeedButton
      Left = 412
      Top = 128
      Width = 21
      Height = 21
      Caption = '...'
      OnClick = SpeedButtonSelectFileClick
    end
    object LabelTimePos: TStaticText
      Left = 0
      Top = 0
      Width = 67
      Height = 17
      Caption = '(10) 00:00:00'
      TabOrder = 6
    end
    object LabelCurrentAction: TStaticText
      Left = 121
      Top = 0
      Width = 94
      Height = 17
      Caption = 'LabelCurrentAction'
      TabOrder = 7
    end
    object ButtonStart: TButton
      Left = 169
      Top = 161
      Width = 97
      Height = 31
      Caption = 'Start'
      Default = True
      TabOrder = 4
      OnClick = ButtonStartClick
    end
    object ButtonCancel: TButton
      Left = 289
      Top = 161
      Width = 97
      Height = 31
      Cancel = True
      Caption = 'Cancel'
      TabOrder = 5
      OnClick = ButtonCancelClick
    end
    object EditFileName: TEdit
      Left = 0
      Top = 129
      Width = 409
      Height = 21
      TabOrder = 3
    end
    object GroupBoxVideoFileTracks: TGroupBox
      Left = 0
      Top = 23
      Width = 161
      Height = 89
      Caption = 'Video File Tracks'
      TabOrder = 0
      object CheckBoxVideo: TCheckBox
        Left = 8
        Top = 18
        Width = 97
        Height = 17
        Caption = 'Video'
        Checked = True
        State = cbChecked
        TabOrder = 0
      end
      object CheckBoxAudio: TCheckBox
        Left = 8
        Top = 40
        Width = 96
        Height = 17
        Caption = 'Audio'
        Checked = True
        State = cbChecked
        TabOrder = 1
      end
      object CheckBoxMeta: TCheckBox
        Left = 8
        Top = 62
        Width = 111
        Height = 17
        Caption = 'Meta Info'
        Checked = True
        State = cbChecked
        TabOrder = 2
      end
    end
    object GroupBoxAdditionalFiles: TGroupBox
      Left = 185
      Top = 23
      Width = 249
      Height = 50
      Caption = 'Additional Files'
      TabOrder = 1
      object CheckBoxSRTSubtitles: TCheckBox
        Left = 8
        Top = 16
        Width = 185
        Height = 17
        Caption = 'Subtitles (.srt)'
        TabOrder = 0
      end
    end
    object ButtonSubtitle: TButton
      Left = 185
      Top = 86
      Width = 145
      Height = 25
      Caption = 'Setup subtitles template'
      TabOrder = 2
      OnClick = ButtonSubtitleClick
    end
  end
  object PanelCenter: TPanel
    Left = 0
    Top = 0
    Width = 564
    Height = 179
    Align = alClient
    BevelOuter = bvNone
    TabOrder = 0
    object ListView: TListView
      Left = 0
      Top = 0
      Width = 539
      Height = 169
      Align = alClient
      Columns = <
        item
          AutoSize = True
          Caption = 'FileName'
        end
        item
          Alignment = taRightJustify
          Caption = 'Start - Finish'
          Width = 110
        end
        item
          Alignment = taRightJustify
          Caption = 'Speed'
          Width = 70
        end
        item
          Alignment = taRightJustify
          Caption = 'Duration'
          Width = 65
        end>
      GridLines = True
      HideSelection = False
      MultiSelect = True
      OwnerData = True
      ReadOnly = True
      RowSelect = True
      PopupMenu = PopupMenuCropList
      TabOrder = 0
      ViewStyle = vsReport
      OnData = ListViewData
      OnDblClick = N_EditClick
    end
    object PanelRight: TPanel
      Left = 539
      Top = 0
      Width = 25
      Height = 169
      Align = alRight
      AutoSize = True
      BevelOuter = bvNone
      TabOrder = 1
      object ButtonAdd: TSpeedButton
        Left = 0
        Top = 0
        Width = 25
        Height = 25
        Caption = '+'
      end
      object ButtonDel: TSpeedButton
        Left = 0
        Top = 25
        Width = 25
        Height = 25
        Caption = '-'
      end
      object ButtonUp: TSpeedButton
        Left = 0
        Top = 82
        Width = 25
        Height = 25
        Caption = '^'
      end
      object ButtonDown: TSpeedButton
        Left = 0
        Top = 107
        Width = 25
        Height = 25
        Caption = 'v'
      end
      object ButtonEdit: TSpeedButton
        Left = 0
        Top = 140
        Width = 25
        Height = 25
        Caption = 'E'
      end
      object ButtonClear: TSpeedButton
        Left = 0
        Top = 50
        Width = 25
        Height = 25
        Caption = '.'
      end
    end
    object PanelSpace: TPanel
      Left = 0
      Top = 169
      Width = 564
      Height = 10
      Align = alBottom
      BevelOuter = bvNone
      TabOrder = 2
    end
  end
  object SaveDialog: TSaveDialog
    DefaultExt = 'mp4'
    Filter = 'MP4 and MOV files|*.mp4;*.mov|Any Files|*.*'
    Options = [ofOverwritePrompt, ofEnableSizing]
    Left = 240
    Top = 30
  end
  object PopupMenuCropList: TPopupMenu
    AutoHotkeys = maManual
    OnPopup = PopupMenuCropListPopup
    Left = 200
    Top = 32
    object N_Edit: TMenuItem
      Caption = 'N_Edit'
      Default = True
      ShortCut = 13
      OnClick = N_EditClick
    end
    object N_Speed: TMenuItem
      Caption = 'N_Speed'
      OnClick = N_SpeedClick
    end
    object N_NoIntersection: TMenuItem
      Caption = 'N_NoIntersection'
      OnClick = N_NoIntersectionClick
    end
    object N1: TMenuItem
      Caption = '-'
    end
    object N_Up: TMenuItem
      Caption = 'N_Up'
      ShortCut = 32806
      OnClick = N_UpClick
    end
    object N_Down: TMenuItem
      Caption = 'N_Down'
      ShortCut = 32808
      OnClick = N_DownClick
    end
    object N2: TMenuItem
      Caption = '-'
    end
    object N_FileList_Add: TMenuItem
      Caption = 'N_FileList_Add'
      ShortCut = 45
      OnClick = N_FileList_AddClick
    end
    object N_FileList_Del: TMenuItem
      Caption = 'N_FileList_Del'
      ShortCut = 46
      OnClick = N_FileList_DelClick
    end
    object N_FileList_Clear: TMenuItem
      Caption = 'N_FileList_Clear'
      ShortCut = 16462
      OnClick = N_FileList_ClearClick
    end
  end
end
