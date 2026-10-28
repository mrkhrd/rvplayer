object FormGPSLog: TFormGPSLog
  Left = 570
  Top = 260
  Width = 575
  Height = 627
  Caption = 'GPS Log'
  Color = clBtnFace
  Font.Charset = DEFAULT_CHARSET
  Font.Color = clWindowText
  Font.Height = -11
  Font.Name = 'MS Sans Serif'
  Font.Style = []
  KeyPreview = True
  OldCreateOrder = False
  OnClose = FormClose
  OnCreate = FormCreate
  OnShow = FormShow
  PixelsPerInch = 96
  TextHeight = 13
  object PanelBottom: TPanel
    Left = 0
    Top = 461
    Width = 559
    Height = 128
    Align = alBottom
    BevelOuter = bvNone
    TabOrder = 1
    object CheckListBoxConverters: TCheckListBox
      Left = 0
      Top = 0
      Width = 367
      Height = 128
      Align = alClient
      ItemHeight = 13
      TabOrder = 0
    end
    object PanelRight: TPanel
      Left = 367
      Top = 0
      Width = 192
      Height = 128
      Align = alRight
      BevelOuter = bvNone
      TabOrder = 1
      object ButtonAdd: TSpeedButton
        Left = 6
        Top = 6
        Width = 25
        Height = 25
      end
      object ButtonDel: TSpeedButton
        Left = 6
        Top = 37
        Width = 25
        Height = 25
      end
      object ButtonClear: TSpeedButton
        Left = 6
        Top = 68
        Width = 25
        Height = 25
      end
      object ButtonDelDisk: TSpeedButton
        Left = 6
        Top = 99
        Width = 25
        Height = 25
      end
      object ButtonConvert: TButton
        Left = 39
        Top = 75
        Width = 146
        Height = 48
        Caption = 'Convert'
        TabOrder = 0
        OnClick = ButtonConvertClick
      end
      object CheckBoxShowPath: TCheckBox
        Left = 39
        Top = 25
        Width = 155
        Height = 17
        TabStop = False
        Caption = 'Show track on map'
        Checked = True
        State = cbChecked
        TabOrder = 2
        OnClick = ListViewFilesClick
      end
      object CheckBoxAll: TCheckBox
        Left = 39
        Top = 5
        Width = 155
        Height = 17
        TabStop = False
        Caption = 'Select all'
        TabOrder = 1
        OnClick = CheckBoxAllClick
      end
      object CheckBoxJoin: TCheckBox
        Left = 39
        Top = 45
        Width = 155
        Height = 17
        TabStop = False
        Caption = 'Join to one file'
        TabOrder = 3
      end
    end
  end
  object ListViewFiles: TListView
    Left = 0
    Top = 0
    Width = 559
    Height = 461
    Align = alClient
    Checkboxes = True
    Columns = <
      item
        Caption = 'File'
        Width = 350
      end
      item
        Alignment = taRightJustify
        Caption = 'Points'
        Width = 55
      end
      item
        Alignment = taRightJustify
        Caption = 'Diration'
        Width = 70
      end
      item
        Alignment = taRightJustify
        Caption = 'Track, km'
        Width = 60
      end>
    ColumnClick = False
    GridLines = True
    HideSelection = False
    MultiSelect = True
    ReadOnly = True
    RowSelect = True
    PopupMenu = PopupMenuGPSLog
    TabOrder = 0
    ViewStyle = vsReport
    OnClick = ListViewFilesClick
    OnDblClick = N_FileList_ShowExplorerClick
    OnSelectItem = ListViewFilesSelectItem
  end
  object OpenDialog: TOpenDialog
    DefaultExt = 'log'
    Filter = 'GPS Logs (.log)|*.log'
    Options = [ofHideReadOnly, ofAllowMultiSelect, ofEnableSizing]
    Left = 232
    Top = 184
  end
  object PopupMenuGPSLog: TPopupMenu
    AutoHotkeys = maManual
    Left = 160
    Top = 120
    object N_FileList_ShowExplorer: TMenuItem
      Caption = 'N_FileList_ShowExplorer'
      Default = True
      ShortCut = 13
      OnClick = N_FileList_ShowExplorerClick
    end
    object N7: TMenuItem
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
    object N4: TMenuItem
      Caption = '-'
    end
    object N_FileList_DelDisk: TMenuItem
      Caption = 'N_FileList_DelDisk'
      ShortCut = 8238
      OnClick = N_FileList_DelDiskClick
    end
    object N_FileList_DelDiskLogs: TMenuItem
      Caption = 'N_FileList_DelDiskLogs'
      OnClick = N_FileList_DelDiskLogsClick
    end
    object N_Space: TMenuItem
      Caption = 'Space'
      ShortCut = 32
      Visible = False
      OnClick = N_SpaceClick
    end
  end
  object TimerDrawRoute: TTimer
    Enabled = False
    Interval = 500
    OnTimer = TimerDrawRouteTimer
    Left = 112
    Top = 240
  end
end
