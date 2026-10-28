object FormSelectFolder: TFormSelectFolder
  Left = 779
  Top = 410
  Width = 365
  Height = 434
  Caption = 'Select Folder'
  Color = clBtnFace
  Constraints.MinHeight = 100
  Constraints.MinWidth = 200
  Font.Charset = DEFAULT_CHARSET
  Font.Color = clWindowText
  Font.Height = -11
  Font.Name = 'MS Sans Serif'
  Font.Style = []
  OldCreateOrder = True
  OnCreate = FormCreate
  OnResize = FormResize
  OnShow = FormShow
  PixelsPerInch = 96
  TextHeight = 13
  object ShellTreeView: TShellTreeView
    Left = 0
    Top = 0
    Width = 349
    Height = 350
    ObjectTypes = [otFolders]
    Root = 'rfDesktop'
    UseShellImages = True
    Align = alClient
    AutoRefresh = True
    Color = clWhite
    HideSelection = False
    Indent = 19
    ParentColor = False
    RightClickSelect = True
    ShowRoot = False
    TabOrder = 1
    OnChange = ShellTreeViewChange
  end
  object Panel: TPanel
    Left = 0
    Top = 371
    Width = 349
    Height = 25
    Align = alBottom
    BevelOuter = bvNone
    TabOrder = 0
    object ButtonShowInTree: TButton
      Left = 80
      Top = 0
      Width = 89
      Height = 25
      Caption = 'Show'
      TabOrder = 1
      OnClick = ButtonShowInTreeClick
    end
    object ButtonOk: TButton
      Left = 0
      Top = 0
      Width = 75
      Height = 25
      Caption = 'Ok'
      Default = True
      TabOrder = 0
      OnClick = ButtonOkClick
    end
    object ButtonCancel: TButton
      Left = 272
      Top = 0
      Width = 75
      Height = 25
      Cancel = True
      Caption = 'Cancel'
      TabOrder = 3
      OnClick = ButtonCancelClick
    end
    object ButtonAddFolder: TButton
      Left = 176
      Top = 0
      Width = 89
      Height = 25
      Caption = 'Add Folder'
      TabOrder = 2
      OnClick = ButtonAddFolderClick
    end
  end
  object PanelEdit: TPanel
    Left = 0
    Top = 350
    Width = 349
    Height = 21
    Align = alBottom
    BevelOuter = bvNone
    TabOrder = 2
    object Edit: TEdit
      Left = 0
      Top = 0
      Width = 121
      Height = 21
      TabOrder = 0
      OnChange = EditChange
    end
  end
end
